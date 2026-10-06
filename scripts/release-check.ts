// Checks the release workflow runs before publishing (`.github/workflows/release.yml`):
//   bun scripts/release-check.ts versions <tag>
//     The tag vX.Y.Z matches the version in package.json, tauri.conf.json, Cargo.toml and
//     Cargo.lock's echo entry.
//   bun scripts/release-check.ts signature <installer> <installer.sig> <version>
//     The installer's updater signature verifies against the public key in tauri.conf.json
//     and was made for <version>, so a wrong signing key fails the release instead of
//     shipping an update no installed Echo accepts.

import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

interface VersionFiles {
  packageJson: string;
  tauriConf: string;
  cargoToml: string;
  cargoLock: string;
}

/** Echo's version as each of the four files states it (undefined when not found). */
function versionsOf(files: VersionFiles): Record<string, string | undefined> {
  return {
    "package.json": (JSON.parse(files.packageJson) as { version?: string }).version,
    "src-tauri/tauri.conf.json": (JSON.parse(files.tauriConf) as { version?: string }).version,
    "src-tauri/Cargo.toml": /^\[package\][^[]*?\nversion\s*=\s*"([^"]+)"/m.exec(
      files.cargoToml,
    )?.[1],
    "src-tauri/Cargo.lock": /^\[\[package\]\]\r?\nname = "echo"\r?\nversion = "([^"]+)"/m.exec(
      files.cargoLock,
    )?.[1],
  };
}

/** One error per file whose version does not match the tag vX.Y.Z; empty when all match. */
export function checkVersions(files: VersionFiles, tag: string): string[] {
  return Object.entries(versionsOf(files))
    .filter(([, version]) => `v${version}` !== tag)
    .map(([file, version]) => `Tag ${tag} does not match version ${version} in ${file}`);
}

/** Line `line` (from 0) of a minisign key or signature file. */
function decodeLine(text: string, line: number, what: string): string {
  const lines = text.split(/\r?\n/);
  const value = lines[line];
  if (value === undefined) throw new Error(`The ${what} is incomplete`);
  return value;
}

/**
 * Verifies a Tauri updater signature (a minisign signature, base64-encoded as a whole, like
 * the public key) of `data`, the way the updater does with `requireSignedVersion`: the
 * signature must verify against `pubkey` and its trusted comment must name `version`.
 */
export function verifyUpdateSignature(
  data: Buffer,
  signature: string,
  pubkey: string,
  version: string,
): void {
  const keyText = Buffer.from(pubkey.trim(), "base64").toString("utf8");
  const key = Buffer.from(decodeLine(keyText, 1, "public key"), "base64");
  if (key.length !== 42 || key.subarray(0, 2).toString() !== "Ed") {
    throw new Error("The public key is not a minisign Ed25519 key");
  }
  const signatureText = Buffer.from(signature.trim(), "base64").toString("utf8");
  const sig = Buffer.from(decodeLine(signatureText, 1, "signature"), "base64");
  const commentLine = decodeLine(signatureText, 2, "signature");
  const globalSig = Buffer.from(decodeLine(signatureText, 3, "signature"), "base64");
  const algorithm = sig.subarray(0, 2).toString();
  if (sig.length !== 74 || (algorithm !== "ED" && algorithm !== "Ed")) {
    throw new Error("The signature is not a minisign Ed25519 signature");
  }
  if (!sig.subarray(2, 10).equals(key.subarray(2, 10))) {
    throw new Error("The package was signed with another key than the one in tauri.conf.json");
  }
  const publicKey = createPublicKey({
    key: { kty: "OKP", crv: "Ed25519", x: key.subarray(10).toString("base64url") },
    format: "jwk",
  });
  const message = algorithm === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (!verify(null, message, publicKey, sig.subarray(10))) {
    throw new Error("The package's signature does not verify");
  }
  const prefix = "trusted comment: ";
  if (!commentLine.startsWith(prefix)) throw new Error("The signature has no trusted comment");
  const comment = commentLine.slice(prefix.length);
  const signed = Buffer.concat([sig.subarray(10), Buffer.from(comment, "utf8")]);
  if (!verify(null, signed, publicKey, globalSig)) {
    throw new Error("The signature's trusted comment does not verify");
  }
  const signedVersion = comment
    .split("\t")
    .find((field) => field.startsWith("version:"))
    ?.slice("version:".length);
  if (signedVersion?.replace(/^v/, "") !== version) {
    throw new Error(`The signature is for version ${signedVersion ?? "(none)"}, not ${version}`);
  }
}

function main(args: string[]) {
  const root = fileURLToPath(new URL("..", import.meta.url));
  const read = (...path: string[]) => readFileSync(join(root, ...path), "utf8");
  const [command, ...rest] = args;
  const [first = ""] = rest;
  if (command === "versions" && rest.length === 1) {
    const errors = checkVersions(
      {
        packageJson: read("package.json"),
        tauriConf: read("src-tauri", "tauri.conf.json"),
        cargoToml: read("src-tauri", "Cargo.toml"),
        cargoLock: read("src-tauri", "Cargo.lock"),
      },
      first,
    );
    if (errors.length > 0) throw new Error(errors.join("\n"));
    console.log(`All four files are at ${first}`);
  } else if (command === "signature" && rest.length === 3) {
    const [installer, signatureFile, version] = rest as [string, string, string];
    const conf = JSON.parse(read("src-tauri", "tauri.conf.json")) as {
      plugins: { updater: { pubkey: string } };
    };
    verifyUpdateSignature(
      readFileSync(installer),
      readFileSync(signatureFile, "utf8"),
      conf.plugins.updater.pubkey,
      version,
    );
    console.log(`${installer} is signed for ${version} with Echo's updater key`);
  } else {
    throw new Error(
      "Usage: release-check.ts versions <tag> | signature <installer> <installer.sig> <version>",
    );
  }
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}

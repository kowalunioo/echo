import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { bumpFiles } from "./bump";
import { checkVersions, verifyUpdateSignature } from "./release-check";

const root = join(__dirname, "..");
const read = (...path: string[]) => readFileSync(join(root, ...path), "utf8");
const fixtures = join(root, "src-tauri", "src", "updater", "fixtures");
const fixture = (name: string) => readFileSync(join(fixtures, name));

describe("checkVersions", () => {
  const files = {
    packageJson: read("package.json"),
    tauriConf: read("src-tauri", "tauri.conf.json"),
    cargoToml: read("src-tauri", "Cargo.toml"),
    cargoLock: read("src-tauri", "Cargo.lock"),
  };

  it("accepts a tag that matches the version in all four files", () => {
    const bumped = bumpFiles(files, "9.8.7");
    expect(checkVersions(bumped, "v9.8.7")).toEqual([]);
  });

  it("rejects a tag that does not match", () => {
    const bumped = bumpFiles(files, "9.8.7");
    expect(checkVersions(bumped, "v9.8.8")).toHaveLength(4);
  });

  it("names each file whose version differs", () => {
    const bumped = bumpFiles(files, "9.8.7");
    const stale = { ...bumped, cargoLock: files.cargoLock, cargoToml: files.cargoToml };
    const errors = checkVersions(stale, "v9.8.7");
    expect(errors).toHaveLength(2);
    expect(errors.join("\n")).toContain("Cargo.toml");
    expect(errors.join("\n")).toContain("Cargo.lock");
  });
});

describe("verifyUpdateSignature", () => {
  const pubkey = fixture("test-key.pub").toString("utf8").trim();
  const signature = fixture("package.bin.sig").toString("utf8").trim();
  const data = fixture("package.bin");

  it("accepts a package signed with the key for the announced version", () => {
    expect(() => {
      verifyUpdateSignature(data, signature, pubkey, "0.2.0");
    }).not.toThrow();
  });

  it("rejects a package signed with another key", () => {
    const other = fixture("package.bin.other-key.sig").toString("utf8").trim();
    expect(() => {
      verifyUpdateSignature(data, other, pubkey, "0.2.0");
    }).toThrow();
  });

  it("rejects a changed package", () => {
    const changed = Buffer.concat([data, Buffer.from("x")]);
    expect(() => {
      verifyUpdateSignature(changed, signature, pubkey, "0.2.0");
    }).toThrow("does not verify");
  });

  it("rejects a signature without the release's version", () => {
    expect(() => {
      verifyUpdateSignature(data, signature, pubkey, "0.3.0");
    }).toThrow("version");
  });

  it("reads Echo's own public key from tauri.conf.json", () => {
    const conf = JSON.parse(read("src-tauri", "tauri.conf.json")) as {
      plugins: { updater: { pubkey: string } };
    };
    // Echo's key did not sign the fixture: a wrong-key failure, not a parse error.
    expect(() => {
      verifyUpdateSignature(data, signature, conf.plugins.updater.pubkey, "0.2.0");
    }).toThrow("another key");
  });
});

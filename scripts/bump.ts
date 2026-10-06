// Bumps Echo's version everywhere it is written down, for a release:
//   bun run bump [patch|minor|major|x.y.z]     (default: patch)
// Updates package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml and src-tauri/Cargo.lock,
// then prints "Bumped A -> B". The release workflow checks that the tag matches all four.

import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** The version after `bump` ("patch", "minor", "major" or an explicit x.y.z) of `current`. */
export function nextVersion(current: string, bump: string): string {
  const parts = SEMVER.exec(current);
  if (!parts) throw new Error(`The current version ${current} is not x.y.z`);
  const [major, minor, patch] = parts.slice(1).map(Number) as [number, number, number];
  switch (bump) {
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "major":
      return `${major + 1}.0.0`;
    default: {
      const target = SEMVER.exec(bump);
      if (!target) throw new Error(`Unknown bump "${bump}": use patch, minor, major or x.y.z`);
      // The updater never downgrades, so a release must be newer.
      if (compare(target.slice(1).map(Number), [major, minor, patch]) <= 0) {
        throw new Error(`${bump} is not newer than ${current}`);
      }
      return bump;
    }
  }
}

function compare(a: number[], b: number[]): number {
  for (let i = 0; i < 3; i++) {
    const diff = (a[i] ?? 0) - (b[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** Replaces exactly one match of `pattern` (whose group 1 is the text before the version). */
function replaceOnce(text: string, pattern: RegExp, version: string, file: string): string {
  const matches = text.match(new RegExp(pattern.source, pattern.flags.replace("g", "") + "g"));
  if (matches?.length !== 1) throw new Error(`Expected one version in ${file}`);
  return text.replace(pattern, (_match, before: string, after: string) => {
    return `${before}${version}${after}`;
  });
}

/** The four files' new contents, given their current contents. */
export function bumpFiles(
  files: { packageJson: string; tauriConf: string; cargoToml: string; cargoLock: string },
  version: string,
) {
  return {
    packageJson: replaceOnce(
      files.packageJson,
      /^(\s*"version":\s*")[^"]+(",?)$/m,
      version,
      "package.json",
    ),
    tauriConf: replaceOnce(
      files.tauriConf,
      /^(\s*"version":\s*")[^"]+(",?)$/m,
      version,
      "tauri.conf.json",
    ),
    cargoToml: replaceOnce(
      files.cargoToml,
      /^(\[package\][^[]*?\nversion\s*=\s*")[^"]+(")/m,
      version,
      "Cargo.toml",
    ),
    cargoLock: replaceOnce(
      files.cargoLock,
      /^(\[\[package\]\]\r?\nname = "echo"\r?\nversion = ")[^"]+(")/m,
      version,
      "Cargo.lock",
    ),
  };
}

/** The version in package.json. */
export function currentVersion(packageJson: string): string {
  return (JSON.parse(packageJson) as { version: string }).version;
}

function main() {
  const root = fileURLToPath(new URL("..", import.meta.url));
  const paths = {
    packageJson: join(root, "package.json"),
    tauriConf: join(root, "src-tauri", "tauri.conf.json"),
    cargoToml: join(root, "src-tauri", "Cargo.toml"),
    cargoLock: join(root, "src-tauri", "Cargo.lock"),
  };
  const read = (path: string) => readFileSync(path, "utf8");
  const files = {
    packageJson: read(paths.packageJson),
    tauriConf: read(paths.tauriConf),
    cargoToml: read(paths.cargoToml),
    cargoLock: read(paths.cargoLock),
  };
  const current = currentVersion(files.packageJson);
  const version = nextVersion(current, process.argv[2] ?? "patch");
  const bumped = bumpFiles(files, version);
  for (const key of Object.keys(paths) as (keyof typeof paths)[]) {
    writeFileSync(paths[key], bumped[key]);
  }
  console.log(`Bumped ${current} -> ${version}`);
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}

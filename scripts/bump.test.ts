import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { bumpFiles, currentVersion, nextVersion } from "./bump";

const root = join(__dirname, "..");
const read = (...path: string[]) => readFileSync(join(root, ...path), "utf8");

describe("nextVersion", () => {
  it("bumps patch, minor and major", () => {
    expect(nextVersion("0.1.9", "patch")).toBe("0.1.10");
    expect(nextVersion("0.1.9", "minor")).toBe("0.2.0");
    expect(nextVersion("0.1.9", "major")).toBe("1.0.0");
  });

  it("accepts an explicit newer version and refuses an older or equal one", () => {
    expect(nextVersion("0.1.9", "0.3.0")).toBe("0.3.0");
    expect(() => nextVersion("0.2.0", "0.1.10")).toThrow("not newer");
    expect(() => nextVersion("0.2.0", "0.2.0")).toThrow("not newer");
    expect(() => nextVersion("0.2.0", "next")).toThrow("Unknown bump");
  });
});

describe("bumpFiles", () => {
  const files = {
    packageJson: read("package.json"),
    tauriConf: read("src-tauri", "tauri.conf.json"),
    cargoToml: read("src-tauri", "Cargo.toml"),
    cargoLock: read("src-tauri", "Cargo.lock"),
  };

  it("changes Echo's version in all four files and nothing else", () => {
    const current = currentVersion(files.packageJson);
    const bumped = bumpFiles(files, "9.8.7");

    expect(currentVersion(bumped.packageJson)).toBe("9.8.7");
    expect(currentVersion(bumped.tauriConf)).toBe("9.8.7");
    expect(bumped.cargoToml).toMatch(/^\[package\]\nname = "echo"\nversion = "9\.8\.7"$/m);
    expect(bumped.cargoLock).toMatch(/name = "echo"\r?\nversion = "9\.8\.7"/);

    for (const key of Object.keys(files) as (keyof typeof files)[]) {
      // Exactly one line differs in each file.
      const before = files[key].split("\n");
      const after = bumped[key].split("\n");
      expect(after).toHaveLength(before.length);
      const changed = before.filter((line, i) => line !== after[i]);
      expect(changed, key).toHaveLength(1);
      expect(changed[0], key).toContain(current);
    }
  });
});

describe("bun run bump", () => {
  it("rewrites a copy of the project and prints the change", () => {
    const copy = mkdtempSync(join(tmpdir(), "echo-bump-"));
    mkdirSync(join(copy, "scripts"));
    mkdirSync(join(copy, "src-tauri"));
    cpSync(join(root, "scripts", "bump.ts"), join(copy, "scripts", "bump.ts"));
    cpSync(join(root, "package.json"), join(copy, "package.json"));
    for (const file of ["tauri.conf.json", "Cargo.toml", "Cargo.lock"]) {
      cpSync(join(root, "src-tauri", file), join(copy, "src-tauri", file));
    }
    const current = currentVersion(read("package.json"));

    const output = execFileSync("bun", [join(copy, "scripts", "bump.ts"), "minor"], {
      encoding: "utf8",
    });

    const next = nextVersion(current, "minor");
    expect(output.trim()).toBe(`Bumped ${current} -> ${next}`);
    expect(currentVersion(readFileSync(join(copy, "package.json"), "utf8"))).toBe(next);
    expect(readFileSync(join(copy, "src-tauri", "Cargo.toml"), "utf8")).toContain(
      `version = "${next}"`,
    );
    rmSync(copy, { recursive: true, force: true });
  });
});

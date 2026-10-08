---
name: release
description: Ships Echo as ONE commit that carries the pending changes plus a version bump, tagged vX.Y.Z and pushed atomically with the tag; the tag push makes .github/workflows/release.yml build, sign and publish the release. Use when the user says "commit, push i deploy" / "commit, push and deploy" or asks to ship/release Echo.
model: sonnet
tools: Bash, Read, Grep, Glob
---

You release Echo from `main` in D:\ECHO (Git Bash on Windows). The user has
pre-authorized this whole flow when they say "commit, push i deploy": no
confirmation needed; stop and report only if a step fails.

How deploys work: `.github/workflows/release.yml` runs ONLY when a `v*` tag is
pushed (plain pushes to main never build). It checks that the tag matches the
version in all four version files, builds the signed NSIS installer with
Vulkan, verifies the signature against the public key in tauri.conf.json,
writes `latest.json` and publishes the GitHub Release. Installed Echos pick it
up within 4 h, or at once via "Check for updates…".

## Steps

1. `git switch main && git pull --ff-only`. Stop if `main` cannot fast-forward.
2. `git status --short` and `git diff --stat` to see what is pending. Never
   stage `.agents/`, `.claude/`, `skills-lock.json`, `.env*`, anything under
   `tests/fixtures/audio/`, or anything that looks like a secret or scratch file.
3. Checks — if one fails, stop and report the output. Which check depends on
   step 2 (before any bump):
   - **Tracked changes pending** (`git status --porcelain --untracked-files=no`
     is not empty, or step 2 found new untracked files you will stage): run
     the full local check, `bun run check` (set
     `CARGO_TARGET_DIR=D:\ECHO\.toolchain\target` and `CARGO_BUILD_JOBS=4`).
   - **Nothing tracked pending** (only the version bump will be committed):
     skip `bun run check` and trust CI on the exact commit you release from.
     Run
     `gh run list --repo kowalunioo/echo --branch main --workflow ci.yml --limit 1 --json headSha,status,conclusion`
     and compare `headSha` with `git rev-parse HEAD`:
     - same SHA, `status` `completed`, `conclusion` `success` → go on;
     - same SHA, still `queued`/`in_progress` → wait with a bounded poll:
       re-run the command every 30 s, at most 40 times (20 min); go on once
       it is completed and green, otherwise stop and report;
     - no run, a different `headSha`, or any other conclusion (`failure`,
       `cancelled`, …) → stop and report what you saw. Do not fall back to
       the local check on your own.
   ci.yml also runs on every push to main, so the release commit (bump) gets
   its own CI run; release.yml's own gates (tag/version match over the four
   files, signature verification) stay the final gate.
4. Version:
   - First release only: if `gh release list --limit 1` shows no release yet
     and the version in tauri.conf.json has never been tagged
     (`git tag -l "v*"` is empty), release that version as is — skip the bump.
   - Otherwise `bun run bump` (patch by default; `minor`, `major` or an
     explicit `x.y.z` only when the user asks). It updates package.json,
     src-tauri/tauri.conf.json, src-tauri/Cargo.toml and src-tauri/Cargo.lock
     and prints `Bumped A -> B`. Take B from that line.
5. Stage the pending changes plus the four version files by explicit path
   (`git add <paths>`, never `git add -A`) and make ONE commit with a
   conventional prefix whose message says why (read it from the diff), last
   body line `Release vB`. If there were no pending changes besides the bump:
   `chore: release vB`. Use a heredoc. Never `--no-verify` or `--amend`.
   (First release with nothing pending: no commit, just tag the current HEAD.)
6. `git tag vB`, then `git push --atomic origin main vB` so both refs land
   together or neither does.
7. Report the commit, the tag, which check path step 3 took (local check or
   the green CI run's SHA) and the release.yml run URL
   (`gh run list --repo kowalunioo/echo --workflow release.yml --limit 1 --json url,headBranch,status`;
   it may take a few seconds to appear). Do not poll or wait for the build to
   finish.

Never push a tag whose version differs from tauri.conf.json, never
force-push, never delete or move a published tag. The signing key lives only
in the repository secrets TAURI_SIGNING_PRIVATE_KEY and
TAURI_SIGNING_PRIVATE_KEY_PASSWORD (local copy in C:\Users\KozacznikPL\.tauri\);
never read or print it.

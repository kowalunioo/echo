# Clean-room rewrite

Echo was originally derived from Handy, so its LICENSE had to carry Handy's copyright. We rewrote it from zero in a new repository so that no derived code remains. The process is a strict clean room: only a dedicated spec-writer agent reads the legacy code and writes behaviour-only specs (no names, structures or snippets from it) into `docs/specs/`; everyone who writes code works only from those specs, the plan and this repo. The legacy repo is archived as `kowalunioo/echo-legacy`.

## Consequences

- Similarity to the legacy code is a defect even when it would be convenient — re-derive from the spec.
- No data compatibility with the old app; settings and history start fresh under the new identifier `com.enloque.echo`.

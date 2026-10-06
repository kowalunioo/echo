# Settings: how they work and how to add one

Behaviour is specified in [`specs/settings-and-first-run.md`](specs/settings-and-first-run.md) (rules 11–15). This page is for implementers.

## Where things live

| Piece | File |
|---|---|
| The model: every setting, its type and its default | `src-tauri/src/settings/mod.rs` |
| Loading, salvage, atomic saving, change notifications | `src-tauri/src/settings/store.rs` (`SettingsStore`) |
| Commands and the `SettingsChanged` event | `src-tauri/src/settings/commands.rs` |
| Frontend store and `useSetting` hook | `src/store/settings.ts` |
| Fake backend used by every frontend test | `src/test/backend.ts` |

The file is `%LOCALAPPDATA%\com.enloque.echo\settings.json`: a JSON object with a `version` and one top-level key per setting, e.g. `{"version": 1, "uiLanguage": "pl", ...}`. On load, every key is validated on its own: an invalid value falls back to its default and the file is rewritten; a missing key gets its default; a file that is not a JSON object is renamed `settings.json.broken` and defaults are used. Keys the running version does not know (written by a newer Echo) are kept and written back unchanged.

## Adding a setting

1. **Rust model.** Add one line to the `settings_model!` list in `src-tauri/src/settings/mod.rs`, with a doc comment naming the owning spec:

   ```rust
   /// How many Transcripts History keeps (`history.md`).
   history_limit: HistoryLimit,
   ```

   The type's `Deserialize` must accept exactly the valid values, because that is what salvage and the `update_settings` command use to reject invalid ones. Enums do this on their own; for ranges or formats use a newtype with `#[serde(try_from = "u32")]` (or similar) and a `TryFrom` that checks the value. Prefer one top-level field per setting over nested objects: salvage and patches work per top-level key.

2. **Default.** Give it its default in `Settings::defaults` (same file). Defaults that depend on the machine (like the UI Language) are computed there from the arguments.

3. **Bindings.** Run `bun run bindings`. The `Settings` and `SettingsPatch` TypeScript types in `src/bindings.ts` now include the field.

4. **Fake backend.** Add the default to `DEFAULT_SETTINGS` in `src/test/backend.ts` (`tsc` reports it as missing until you do).

5. **Tests.** Add a salvage test for an invalid stored value if the type has its own validation (see `store.rs` tests for the pattern).

That is all: loading, salvage, saving, events and the frontend store need no changes.

## Using settings

**Frontend** — read and change one setting in a component (rendered only after settings have loaded):

```tsx
const [limit, setLimit] = useSetting("historyLimit");
// ...
<input value={limit} onChange={(e) => void setLimit(Number(e.target.value))} />
```

Changes show immediately, are saved by the backend at once (no Save button), and come back to every window through the `settingsChanged` event. For a "reset to default" control call `useSettings.getState().reset("historyLimit")`.

**Rust** — the store is in Tauri's managed state:

```rust
let store = app.state::<SettingsStore>();
let limit = store.get().history_limit;
store.update(|s| s.history_limit = new_limit)?;     // saves and notifies
```

To react to changes (re-register a shortcut, relabel the tray menu when the UI Language changes, …), subscribe once at start-up:

```rust
store.subscribe(|old, new| {
    if old.ui_language != new.ui_language { /* relabel */ }
});
```

Listeners run after the change is saved, outside the store's lock, so they may call `store.get()`. When two changes race, listeners may see them out of order; read `store.get()` if you need the latest state.

## Format version

`SETTINGS_FORMAT_VERSION` in `store.rs` only changes when an existing key changes meaning or shape. Adding a setting does not need a new version. When you do bump it, put the conversion from the older shape in `migrate` (same file).

# EZMapa Roadmap

Working plan for the launcher, organized as **Now / Next / Later**. Items move
up as they're scoped and down as priorities shift. Shipped items drop to the
bottom with the release they landed in.

Last reviewed against `main` at v0.3.0.

---

## Now — v0.3.x

- **CI hardening (remaining)** — Clippy (`-D warnings`) and a `windows-latest`
  runner are in CI. Still missing: `cargo fmt --check`, ESLint + Prettier for
  the frontend, and (optionally) a Linux runner for cross-platform compile
  checks.
- **Dead-code cleanup** — remove or wire up unused surfaces the compiler still
  flags: `crosssource/mod.rs` module-level `#![allow(dead_code)]`, a few
  `#[allow(dead_code)]` items in `mojang.rs` / `servers.rs`, and any new
  warnings after the v0.3.0 merge.
- **World delete safety net** — manual world snapshots (backup to `.zip`,
  restore, delete snapshot) already exist on the Worlds tab. Still missing:
  an automatic safety backup before `delete_world` runs.
- **v0.3.0 validation** — dogfood the five cross-source features against live
  Modrinth/CurseForge APIs, real modded servers, and actual game launches;
  file bugs from anything that only shows up outside the sandbox.

## Next

- **Mod update flow polish** — v0.3.0 added cross-platform update checking,
  source badges, per-mod "switch source of truth", and a review banner with
  "Update all". Still open: per-item apply/skip, clearer entry points outside
  the content tabs, and reconciling that UI with the global "Auto-update
  content on launch" setting.
- **Frontend unit tests** — Vitest (drop-in with Vite), starting with the
  crash-analysis rules in `src/lib/crash.ts`: pure functions where a regression
  silently gives players wrong crash advice.
- **Localization** — `src/lib/strings.ts` exists and v0.3.0 features route
  through `t()`, but most of the UI is still hardcoded English. Finish
  extraction opportunistically; add Norwegian as the first locale.
- **README accuracy** — README still claims "resumable" downloads; `net.rs`
  writes to a `.part` temp file but restarts from scratch on failure (no HTTP
  Range resume yet). Align docs or implement resume (see Later).

## Later

- **Cross-platform builds (macOS / Linux)** — the code paths exist
  (`open_url`, keyring, and Java detection are already cfg-gated per OS);
  needs release-workflow matrix entries, testing, and signing story.
- **Download resilience** — resume partially-downloaded files after a network
  drop (HTTP Range) instead of restarting them.
- **Accessibility pass** — keyboard navigation and screen-reader labels across
  modals and the command palette.

---

## Shipped

| Release | Item |
|---------|------|
| v0.3.0 | Blocked CurseForge downloads auto-resolve via Modrinth (hash-verified) |
| v0.3.0 | Cross-platform mod/resource/shader update checking + switch source of truth |
| v0.3.0 | CurseForge modpack export + "Export both" (`.mrpack` + CF pack) |
| v0.3.0 | Create matching instance from modded Forge/NeoForge server ping |
| v0.3.0 | Recommended JVM settings per instance + startup before/after stats |
| v0.3.0 | String table infrastructure (`src/lib/strings.ts`) for future locales |
| v0.2.6 | Microsoft session persistence fix (Windows Credential Manager / keyring) |
| v0.2.6 | Automatic launcher self-update on startup |
| v0.2.6 | Quick Play desktop shortcuts (world / server / instance) |
| v0.2.6 | EZMapa Wrapped — local year-in-review share card |
| v0.2.6 | Import `.mrpack` as a new instance (drag-and-drop + create-instance modal) |
| v0.2.6 | World snapshots — manual backup to zip, restore, delete snapshot |
| v0.2.6 | CI: Clippy (`-D warnings`) + `windows-latest` runner |
| v0.2.5 | Microsoft tokens moved to OS keyring (with migration) |
| v0.2.5 | Zip-slip protection for archive extraction |
| v0.2.5 | Instance export to `.zip` and `.mrpack` |
| v0.2.5 | CI: frontend build + Rust tests on every push |
| v0.2.4 | Content tab pagination + version picker for packs/shaders |
| v0.2.3 | Security hardening (XSS fix, atomic writes), release notes pipeline |
| v0.2.x | Per-instance launch settings (memory, JVM args, window size, env vars, pre/post hooks) |
| v0.2.x | Signed release pipeline, log sharing via mclo.gs, code-splitting |
| v0.2.0 | Server browser, skin library, playtime stats |

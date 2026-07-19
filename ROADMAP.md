# EZMapa Roadmap

Working plan for the launcher, organized as **Now / Next / Later**. Items move
up as they're scoped and down as priorities shift. Shipped items drop to the
bottom with the release they landed in.

Last reviewed for v0.4.2.

---

## Now — v0.3.x

- **CI hardening (remaining)** — frontend build, ESLint, Prettier, Vitest,
  Clippy (`-D warnings`), and Rust tests run on `windows-latest`. Still open:
  `cargo fmt --check` and an optional Linux compile-check runner.
- **Dead-code cleanup** — remove or wire up unused surfaces the compiler still
  flags: `crosssource/mod.rs` module-level `#![allow(dead_code)]`, a few
  `#[allow(dead_code)]` items in `mojang.rs` / `servers.rs`, and any new
  warnings after the v0.3.0 merge.
- **v0.3.0 validation** — dogfood the five cross-source features against live
  Modrinth/CurseForge APIs, real modded servers, and actual game launches;
  file bugs from anything that only shows up outside the sandbox.

## Next

- **Mod update flow polish** — v0.3.0 added cross-platform update checking,
  source badges, per-mod "switch source of truth", and a review banner with
  "Update all". Still open: per-item apply/skip, clearer entry points outside
  the content tabs, and reconciling that UI with the global "Auto-update
  content on launch" setting.
- **Frontend unit tests** — Vitest now covers crash-analysis and instance
  organization rules. Expand into update decisions, settings persistence, and
  critical component interaction tests.
- **Localization** — Norwegian now covers the main navigation, instance library,
  onboarding, and appearance settings. Continue extracting the remaining
  hardcoded English strings opportunistically.

## Later

- **Cross-platform builds (macOS / Linux)** — the code paths exist
  (`open_url`, keyring, and Java detection are already cfg-gated per OS);
  needs release-workflow matrix entries, testing, and signing story.
- **Download resilience** — resume partially-downloaded files after a network
  drop (HTTP Range) instead of restarting them.
- **Accessibility pass** — shared dialogs now have focus trapping, focus
  restoration, semantics, and reduced-motion support. Continue with the command
  palette and icon-only controls.

---

## Shipped

| Release | Item |
|---------|------|
| v0.4.2 | Instance Health Center |
| v0.4.2 | Reviewed transactional content updates + rollback |
| v0.4.2 | Pass the Pack `.ezmapa` export/import |
| v0.4.2 | Dependency-aware removal warnings |
| v0.4.2 | Reversible Pack Doctor isolation pass |
| v0.3.2 | Collapsible instance groups + group-aware search |
| v0.3.2 | Automatic world snapshot before deletion |
| v0.3.2 | Norwegian locale foundation + persisted language setting |
| v0.3.2 | Accessible shared dialogs + reduced-motion support |
| v0.3.2 | Frontend lint/format/test quality gates |
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

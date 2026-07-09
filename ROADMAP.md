# EZMapa Roadmap

Working plan for the launcher, organized as **Now / Next / Later**. Items move
up as they're scoped and down as priorities shift. Shipped items drop to the
bottom with the release they landed in.

---

## Now — v0.2.6

- **Import `.mrpack` as a new instance** — v0.2.5 added `.mrpack` *export*;
  close the loop so a shared pack can come back in. Accept `.mrpack` in the
  drag-and-drop zone (`DropZone.tsx` currently only takes `.zip`) and in the
  create-instance modal. Backend `install_mrpack` already exists — this is
  mostly wiring a "create instance from pack file" flow.
- **CI hardening** — add `cargo clippy -- -D warnings` and `cargo fmt --check`
  to CI; add ESLint + Prettier for the frontend; add a `windows-latest` runner
  (we ship Windows builds but only test on Ubuntu).
- **Dead-code cleanup** — remove or wire up the unused items the compiler
  already flags (`record_play`, `Loader::modrinth_id`,
  `DiscordPresence::clear`, unused `mojang.rs` fields).

## Next

- **World backups** — worlds are the only irreplaceable data in a launcher and
  `delete_world` is currently a one-way door. Add manual "back up world to
  .zip", an automatic safety backup before deletion, and restore. `archive.rs`
  and the instance-export code provide the pieces.
- **Frontend unit tests** — Vitest (drop-in with Vite), starting with the
  crash-analysis rules in `src/lib/crash.ts`: pure functions where a regression
  silently gives players wrong crash advice.
- **Mod update flow polish** — `auto_update_content` exists as a setting;
  surface available updates per instance with a review-before-apply list
  instead of update-on-launch only.

## Later

- **Cross-platform builds (macOS / Linux)** — the code paths exist
  (`open_url`, keyring, and Java detection are already cfg-gated per OS);
  needs release-workflow matrix entries, testing, and signing story.
- **Localization** — extract UI strings; Norwegian first.
- **Download resilience** — resume partially-downloaded files after a network
  drop instead of restarting them.
- **Accessibility pass** — keyboard navigation and screen-reader labels across
  modals and the command palette.

---

## Shipped

| Release | Item |
|---------|------|
| v0.2.5 | Microsoft tokens moved to OS keyring (with migration) |
| v0.2.5 | Zip-slip protection for archive extraction |
| v0.2.5 | Instance export to `.zip` and `.mrpack` |
| v0.2.5 | CI: frontend build + Rust tests on every push |
| v0.2.4 | Content tab pagination + version picker for packs/shaders |
| v0.2.3 | Security hardening (XSS fix, atomic writes), release notes pipeline |
| v0.2.x | Per-instance launch settings (memory, JVM args, window size, env vars, pre/post hooks) |
| v0.2.x | Signed release pipeline, log sharing via mclo.gs, code-splitting |
| v0.2.0 | Server browser, skin library, playtime stats |

# Agent handoff log — 5-feature implementation

Running log for whoever continues this work (human, Cursor, or another agent).
Updated at every milestone. Branch: `claude/ezmapa-feature-planning-lfaqgt`.

## The approved plan (summary)

Five features, one phase per commit, in order. Each phase must compile, pass
`cargo clippy --lib --tests` with **zero warnings**, and pass `cargo test --lib`
before moving on. Full plan details are in the conversation, but each phase's
section below restates what matters.

- **Phase 0** — `src-tauri/src/crosssource/`: shared Modrinth↔CurseForge
  identity resolver (sha1 batch lookup, murmur2 fingerprints, disk cache,
  rate limiting). ✅ DONE
- **Phase 1** — Blocked CurseForge downloads (`downloadUrl: null`) re-sourced
  from Modrinth by exact hash; manual-download summary UI. ✅ DONE
- **Phase 2** — Unified update checking across both platforms (extend
  `modrinth::check_updates`, source badges, "switch source of truth").
- **Phase 3** — Dual-format modpack export (CF manifest zip + mrpack from one
  shared "resolved pack model"; embed/exclude review dialog for
  platform-exclusive mods, license warning, default exclude).
- **Phase 4** — Server browser: decode Forge/NeoForge mod list from server
  ping (`forgeData.d` packed UTF-16 → binary), map mods to
  Modrinth/CurseForge, build a matching instance; best-effort UI copy.
- **Phase 5** — Per-instance JVM auto-tuning (ZGC on Java 21+, Aikar-ish G1
  older; Xmx capped by system RAM) with before/after arg diff + confirm, and
  startup-time measurement from log markers stored per instance.

### Hard constraints (user-imposed)

- NEVER direct-link/scrape CurseForge files the API refuses (`downloadUrl:
  null`). Only allowed workaround: hash-verified identical file from
  Modrinth, or manual download by the user. The old `fallback_url()` forgecdn
  URL-guess in curseforge.rs was **removed** for this reason (approved).
- Reuse the existing download pipeline (`net.rs`) — no second download path.
- All new user-facing strings go through `src/lib/strings.ts` (`t()` /
  `plural()`); Norwegian locale planned.
- Unit tests for hash matching, manifest generation, forgeData decoder.
- One commit per phase; after each phase summarize built/untested/edge cases.

## Environment notes

- Linux container. Tauri system deps were missing; installed with:
  `apt-get install pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev
  libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev
  libjavascriptcoregtk-4.1-dev`.
- Verify with: `cd src-tauri && cargo clippy --lib --tests` (zero warnings
  expected) and `cargo test --lib`.
- Frontend typecheck: `npx tsc --noEmit` from repo root.
- Outbound network: `api.modrinth.com` / `api.curseforge.com` are **blocked**
  by the sandbox proxy (403), so nothing was validated against live APIs —
  request/response shapes are covered by canned-JSON unit tests only.
- `git push` to origin currently fails with 403 (permission). Retry
  periodically; commits exist locally.
- First Linux build generated `src-tauri/gen/schemas/linux-schema.json`;
  schemas are tracked by repo convention, so it's committed.

## Architecture facts you'll need

- Backend: flat modules in `src-tauri/src/`; all Tauri commands in
  `commands.rs`, registered in `lib.rs`; shared `AppState` (`state.rs`) with
  `http` (reqwest), `dirs` (AppDirs layout), and now `crosssource`
  (Resolver). Errors: `error::Error` (thiserror), `Result<T>` alias.
- Downloads: `net::download_one` / `download_many_cancellable` — sha1
  verified, `.part` temp files, `task://progress` events keyed by task id
  (`modpack:<instanceId>` for pack installs).
- Per-instance mod identity: `instances/<id>/ezmapa_index.json`
  (`file name → IndexItem { project_id, provider, fallback? }`), managed in
  `instances.rs` (`record_installs`, `record_modrinth_fallback`,
  `load_index`).
- Instance manifest: `instances/<id>/instance.json` (`models.rs::Instance`),
  game dir at `instances/<id>/minecraft/`.
- Frontend: React+zustand. `src/lib/api.ts` wraps `invoke()` and events;
  `src/store/useStore.ts` holds global state + event listeners; pages in
  `src/pages/`, `InstanceDetailPage.tsx` (~2.6k lines) has the mods/updates/
  servers panels; `src/components/ui.tsx` has Button/Modal/Spinner prims.
- Tests are inline `#[cfg(test)] mod tests` per Rust module.

## Phase 0 — DONE (commit b820787)

Built `src-tauri/src/crosssource/` (mod.rs, cache.rs, limiter.rs, murmur2.rs):

- `Resolver` on `AppState.crosssource`:
  - `resolve_hashes_modrinth(http, &[sha1]) → HashMap<sha1, ModrinthRef>` —
    batch `POST /v2/version_files` (chunks of 800), only accepts the file in
    the returned version whose sha1 matches exactly.
  - `resolve_by_hash`, `resolve_cf_file_to_modrinth(http, CurseforgeRef,
    sha1)` (also caches the CF identity), `resolve_local_files_to_cf(http,
    api_key, &[(sha1, path)])` via `POST /v1/fingerprints/432`,
    `resolve_local_file_to_cf`, `cached(sha1)`.
  - Modrinth→CF works ONLY for local files (murmur2 needs the bytes);
    remote-only direction intentionally unsupported (documented in mod.rs).
- `murmur2.rs`: 32-bit MurmurHash2 seed 1, whitespace bytes 9/10/13/32
  stripped (CF fingerprint algo). Reference vectors generated from an
  independent Python transcription — NOT validated against live CF API.
- `cache.rs`: `crosssource_cache.json` in the app data root; positive
  answers never expire, negatives retried after 7 days; atomic writes via
  `instances::atomic_write` (made pub(crate)).
- `limiter.rs`: token bucket (Modrinth 240/min burst 20, CF 60/min burst
  10), test-driven via synthetic `Instant`s.
- `modrinth::ProjectVersion` gained `project_id: String` (serde default).
- `crosssource/mod.rs` still has a module-level `#![allow(dead_code)]` —
  several APIs are consumed only from Phase 2/3. Remove it once they are.

## Phase 1 — Blocked CF downloads → Modrinth fallback (DONE)

Verified: `cargo clippy --lib --tests` zero warnings, `cargo test --lib`
62/62, `tsc --noEmit` clean, `vite build` clean. Untested against live APIs
(network blocked in sandbox): the CF `/v1/mods` batch shape, the fingerprint
endpoint, and end-to-end blocked-file installs. Unit tests cover the decision
logic, serde shapes, report building, and index round-trips.

### Backend

`curseforge.rs`:
- `fallback_url()` (forgecdn guess) DELETED. `CfFile` gained `mod_id` +
  `sha1()` helper.
- `plan_blocked_file(state, &CfFile) → BlockedPlan::{Modrinth(ModrinthRef),
  Manual}` — resolves CF sha1 on Modrinth via `AppState.crosssource`.
- Single installs (`install_content`/`install_file` → new `CfInstall {
  file_name, deps, modrinth_fallback: Option<ModrinthRef> }`): blocked file
  → Modrinth URL (still verified against **CF's** sha1) or a friendly error
  containing the CF page URL (`mod_page_url()`, uses `links.websiteUrl`,
  falls back to `https://www.curseforge.com/projects/{id}`).
- Dependencies: same fallback, silently skipped if unresolvable (dep install
  was already best-effort). `modrinth::InstalledDep` gained
  `modrinth_fallback: Option<ModrinthRef>`.
- `install_modpack`: pack zip itself blocked → error with page link. Bulk
  files: one batch `resolve_hashes_modrinth` for all null-URL files;
  resolved ones download from Modrinth (CF sha1 check), unresolved collected
  and emitted (with names/links from batch `POST /v1/mods`) as
  `ModpackInstallReport { instance_id, resolved_via_modrinth, blocked:
  [BlockedFileInfo { file_name, mod_name, project_id, file_id, page_url }] }`
  on the **`modpack://report`** event BEFORE downloads start (so the UI badge
  can show during install). Install no longer fails per-file for blocked
  files. After success, all bulk files are recorded in the content index
  (provider "curseforge", project id = mod_id) — modpack installs previously
  recorded nothing — plus fallback details.
- `instances.rs`: `IndexItem` gained `fallback: Option<FallbackSource {
  provider, project_id, version_id }>` (serde-skipped when None, old index
  files still parse); new `record_modrinth_fallback()`.
- `commands.rs`: CF install commands consume `CfInstall`; shared
  `record_cf_install()` records index + fallbacks; `InstallOutcome` gained
  `via_modrinth_fallback: bool` (camelCased to the frontend).

### Frontend

- `src/lib/strings.ts` — NEW minimal i18n table (`t(key, vars)`,
  `plural(n, sing, plur)`), English values; keys added for this phase
  (`install.viaModrinth*`, `blocked.*`).
- `types.ts`: `BlockedFileInfo`, `ModpackInstallReport`,
  `InstallOutcome.viaModrinthFallback?`.
- `api.ts`: `events.onModpackReport` for `modpack://report`.
- `useStore.ts`: `packReports: Record<instanceId, ModpackInstallReport>`,
  `activePackReportId`, `dismissPackReport()`; listener stores reports +
  toasts the auto-resolved count; when a `modpack:<id>` task finishes
  without error and the report has blocked files, sets `activePackReportId`
  to open the dialog.
- `BlockedModsModal.tsx` — NEW global modal (mounted in App.tsx) listing
  blocked mods with "Open CurseForge page" buttons (via `api.openUrl`),
  "Open mods folder", dismiss.
- `InstanceCard.tsx`: green "N via Modrinth" badge under the install
  progress bar while installing.

### Tests added in Phase 1

- `curseforge.rs::tests`: sha1 extraction/lowercase, missing-sha1 → Manual,
  bulk null-URL deserialization, blocked-report rows (site link vs
  `/projects/{id}` fallback), `parse_modloader`.
- `instances.rs::index_tests`: fallback round-trip keeps CF identity,
  untracked-file fallback creates entry, legacy index JSON (no `fallback`
  field) parses, reinstall clears stale fallback marker.

## Phases 2–5 — NOT STARTED

Key design decisions already made (see plan summary above), plus:
- Phase 2 extends `modrinth::check_updates` in place; `ModUpdate` gains
  `source` + ids (serde-defaulted); winner picked by release date
  (`date_published` vs CF `fileDate`); "switch source" = new command
  rewriting the content-index entry.
- Phase 3: new `export.rs` with shared resolved-pack model; refactor
  `modrinth::export_mrpack` onto it; new `prepare_pack_export` +
  `export_curseforge_pack` commands; CF zip = manifest.json (manifestType
  "minecraftModpack", manifestVersion 1) + modlist.html + overrides/.
- Phase 4: decoder for `forgeData.d` (15 bits per UTF-16 char → bytes →
  VarInt-framed mod list); test via round-trip with own encoder (real
  payloads unobtainable in sandbox — mark untested); new `server_mods.rs`.
- Phase 5: extend `system.rs` with /proc/meminfo (Linux) + sysctl (macOS)
  probes (repo deliberately avoids a system-info crate); suggestion command
  + arg-merge; startup markers hooked into `launch.rs::spawn_reader`;
  samples in `<instance>/startup_stats.json` keyed by args fingerprint.

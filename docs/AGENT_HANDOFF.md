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
  `modrinth::check_updates`, source badges, "switch source of truth"). ✅ DONE
- **Phase 3** — Dual-format modpack export (CF manifest zip + mrpack from one
  shared "resolved pack model"; embed/exclude review dialog for
  platform-exclusive mods, license warning, default exclude). ✅ DONE
- **Phase 4** — Server browser: decode Forge/NeoForge mod list from server
  ping (`forgeData.d` packed UTF-16 → binary), map mods to
  Modrinth/CurseForge, build a matching instance; best-effort UI copy. ✅ DONE
- **Phase 5** — Per-instance JVM auto-tuning (ZGC on Java 21+, Aikar-ish G1
  older; Xmx capped by system RAM) with before/after arg diff + confirm, and
  startup-time measurement from log markers stored per instance. ✅ DONE

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

## Phase 2 — Unified update checking (DONE)

Verified: clippy zero warnings, 72/72 tests, tsc + vite build clean.
Untested live: CF `POST /v1/mods` (latestFilesIndexes) and
`POST /v1/mods/files` bulk shapes, fingerprint identification end-to-end.
Assumption documented in code: `latestFilesIndexes` is ordered newest-first.

- `curseforge.rs`: `update_candidates(state, &[(sha1, path)], loader,
  game_version) → HashMap<sha1, CfUpdateCandidate>`. Pipeline (3 requests
  total): fingerprint-identify local files → batch `POST /v1/mods` →
  `pick_file_index` filters `latestFilesIndexes` by game version + loader
  (stable releases preferred over beta/alpha; loaderless entries = non-mod
  content match any loader) → bulk `POST /v1/mods/files` for full candidate
  files. Blocked candidates (null URL) are offered only when mirrored on
  Modrinth (via `plan_blocked_file`), else dropped. Best-effort: any failure
  or missing API key → empty map (Modrinth-only behavior).
- `modrinth.rs`: `collect_updatable_hashes` now returns `LocalContent`
  (content_type, file_name, enabled, **path**). `check_updates` merges both
  platforms per file via pure `choose_update(mr, cf, pinned)` — newest
  release date wins (`newer_date`: RFC3339 parse, string-compare fallback);
  the content-index provider is the pin used as tiebreak. `ModUpdate` gained
  `source` (default "modrinth"), `source_project_id`, `source_version_id`,
  `date`, `pinned_provider` — all serde-defaulted so old payloads parse.
- `apply_update` re-anchors the file's index identity to the applied
  update's source (record_installs after migrate_index_entry).
- `instances.rs`: `content_provider_map(state, id)`.
- New command `set_mod_source(instance_id, file_name, provider)`
  (commands.rs, registered in lib.rs): sha1 the local jar, resolve on the
  target platform (Modrinth by hash / CF by fingerprint), rewrite the index
  entry; errors if no identical file exists there. `curseforge::api_key` and
  `loader_type` are now pub(crate).
- Frontend: `ModUpdate` TS type extended; `api.setModSource`;
  `SourceBadge` (green Modrinth / orange CurseForge) on each row of
  `ContentUpdatesBanner`; per-mod ArrowLeftRight button = switch source of
  truth (mods panel only), calls setModSource then re-checks. Strings under
  `updates.*` in strings.ts.
- Tests: `modrinth::tests` (date comparison incl. fractional seconds,
  pin tiebreak, single-platform passthrough, legacy payload deserialization),
  `curseforge::update_tests` (index picking: stable-over-beta, loader/game
  version filters, loaderless entries, response parsing).

## Phase 3 — Dual-format export (DONE)

Verified: clippy zero warnings, 79/79 tests, tsc + vite build clean.
Untested live: whether real CF launchers accept the generated manifest zips
(shape matches the format spec and is unit-tested).

- NEW `src-tauri/src/export.rs`: shared resolved-pack model.
  `build_resolved_pack(state, id) → (Instance, Vec<ResolvedEntry>)` — every
  exportable file (enabled mods/.zip packs/shaders) with sha1+sha512 (one
  streaming pass), size, `modrinth: Option<ModrinthRef>` (batch hash lookup)
  and `curseforge: Option<CurseforgeRef>` (fingerprint; empty without API
  key). `ResolvedEntry::availability()` → both/modrinth/curseforge/none.
- Writers consume the same model:
  - `export_mrpack(state, id, dest, embed: Option<&[String]>)` — Modrinth
    files become hash-verified download refs (locally computed sha1/sha512);
    others embed into `overrides/` only if in the embed list. `embed: None`
    = legacy embed-all (kept for old callers; the UI always sends a list).
  - `export_curseforge_pack(...)` — manifest.json (manifestType
    "minecraftModpack", manifestVersion 1, files[] projectID/fileID/required,
    modLoaders "loader-version", overrides "overrides") + modlist.html
    (HTML-escaped links to /projects/{id}) + overrides/. Errors if files are
    missing from CF and no decision list was passed (never bundles silently).
  - Both include the common overrides (config/, servers.dat, folder-based
    packs) via shared helpers.
- The old export section in modrinth.rs was REMOVED (replaced by a pointer
  comment); commands `export_mrpack` (now takes optional `embed`),
  `export_curseforge_pack`, `prepare_pack_export` (returns
  `PackExportPreview { entries: [{subdir, fileName, availability}] }`).
- Frontend: NEW `src/components/PackExport.tsx` —
  `startPackExport(instanceId, name, format)` callable from anywhere
  (save dialog(s) → preparePackExport → direct export or global review
  modal) + `PackExportModal` mounted in App.tsx; state in useStore
  (`packExport`, `packExporting`). Review dialog: per-file embed checkboxes
  (default excluded), "Embed all", license warning, per-file "Not on
  CurseForge/Modrinth/either" chip. ExportMenu gained "Export as CurseForge
  pack" and "Export both formats"; InstanceCard, CommandPalette, and the
  detail page's exportPack all route through startPackExport (the old
  silent-embed `exportInstanceMrpack` helper in lib/export.ts was removed).
- Tests (`export.rs::tests`): availability classification, embed decisions,
  CF manifest exact shape (incl. neoforge loader id + primary flag),
  undecided-files exclusion, vanilla = no modLoaders, modlist HTML escaping,
  mrpack index refs + selective embedding.

## Phase 4 — Server → matching instance (DONE)

Verified: clippy zero warnings, 90/90 tests, tsc + vite build clean.
UNTESTED (flagged in module docs): the decoder against REAL server payloads —
fixtures are generated from the FML format spec via a test-only encoder
(round-trip). Live resolution (Modrinth/CF lookups) also untested (network
blocked). The `latestFilesIndexes`-newest-first and "NeoForge keeps the
forgeData field" assumptions are documented in code.

- NEW `src-tauri/src/server_mods.rs`:
  - `decode_packed_utf16` — FML `encodeOptimized` inverse: 2 length chars
    (15 bits each, LE) + 15-bits-per-char LSB-first payload; size-capped.
  - `parse_forge_payload` — truncated bool, u16 mod count, per mod VarInt
    (low bit = IGNORESERVERONLY, high bits = channel count), readUtf strings,
    channels skipped. `parse_status_mods(&json)` handles 1.18+ `forgeData.d`,
    1.13–1.17 `forgeData.mods` (plain JSON, "OHNOES" marker), and legacy
    `modinfo.modList`. Loader = "neoforge" iff mod id "neoforge" present.
  - `extract_mc_version` — pulls "1.21.1" out of "Paper 1.21.1" etc.
  - `analyze_server(app, state, address) → ServerModPlan { mc_version,
    loader, truncated, resolved: [PlannedMod], unresolved (with Modrinth
    search links), skipped (platform ids minecraft/forge/neoforge/fml/mcp +
    IGNORESERVERONLY mods) }`. Resolution per mod: Modrinth project by slug →
    search fallback (hit accepted only when normalized slug == modId), skips
    `client_side == "unsupported"`, `pick_version` ranks exact
    version_number, then containment, then filename, else newest (exact:
    false → "closest version" badge); CurseForge fallback via
    `curseforge::find_server_mod_file` (slug filter → normalized-slug text
    search; file whose name carries the version; blocked files only via
    Modrinth mirror). Progress on `task://progress` (`server-analyze:<addr>`).
    Rate-limited via new `Resolver::throttle_modrinth/throttle_curseforge`.
  - `create_instance_from_server` — loader + MC version from the ping,
    loader build = first entry of forge/neoforge list (recommended-first),
    downloads via the shared pipeline under `modpack:<id>` (card shows
    progress; rollback on failure), records all identities in the content
    index. Returns `ServerInstanceOutcome { instance, plan }`.
- `servers.rs`: `ServerStatus.mod_info: Option<ServerModList>` populated in
  `ping_inner`.
- Commands `analyze_server_mods(address)` / `create_instance_from_server(
  name, address)` registered in lib.rs.
- Frontend: `ServerStatus.modInfo` + plan types in types.ts; api wrappers;
  NEW `ServerInstanceModal.tsx` (best-effort warning, plan summary with
  resolved/approx badge/unresolved with search links/skipped, name input,
  create). Servers panel: Boxes-icon "Create matching instance" button on
  rows whose ping shows a forge/neoforge mod list (saved + manual ping rows).
- Tests (`server_mods::tests`, 11): packed round-trip (incl. 0/1-byte and
  1000-byte buffers), full handshake payload with channels + server-only
  flag, NeoForge + truncation detection, FML2 plain JSON, legacy modinfo,
  vanilla → None, corrupt data rejected without panic, MC-version extraction,
  version ranking, slug normalization, URL encoding.
## Phase 5 — JVM auto-tuning + startup measurement (DONE)

Verified: clippy zero warnings, 100/100 tests, tsc + vite build clean.
Untested live: actual game launches (no GPU/game in sandbox), so the
readiness markers ("Sound engine started", "Preparing spawn area",
"Joining world") are validated against known log-line shapes only. macOS
sysctl and Windows RAM paths compile-checked only (Linux path unit-tested).

- `system.rs`: real RAM detection on Linux (/proc/meminfo, parser unit-
  tested) and macOS (sysctl hw.memsize FFI) — kept the file's no-crate
  approach instead of adding `sysinfo`.
- NEW `jvmtune.rs`: `suggest(state, id) → JvmSuggestion { current/suggested
  args + Xmx, java_major, system_ram_mb, mod_count, has_custom_args,
  reasons[] }`. Java major = probed pinned java_path (new
  `java::probe_major`) else `required_major_for_mc` (1.20.5+→21, 1.17+→17,
  else 8). Heap tiers by mod count (2/4/6/8 GB, heavy-pack floor 8 GB via
  `HEAVY_PACK_IDS` on pack_source), capped at half RAM and RAM−2GB, floor
  1 GB. GC flags: ≤16 legacy G1 (repo default), 17–20 Aikar-style client G1,
  21–23 `-XX:+UseZGC -XX:+ZGenerational`, 24+ plain UseZGC (generational
  default, flag obsolete). `merge_args` keeps every non-GC user token
  (`is_gc_token` prefix list) — the merge view honors custom args.
  Suggestion is read-only; applying = the UI fills the settings form and the
  user presses Save (never silently overwrites).
- NEW `startup.rs`: `args_fingerprint(mem, args)` (sha1[..12], whitespace-
  normalized, order-sensitive); `StartupTracker` observes log lines from
  both reader threads (`launch.rs::spawn_reader` gained the param) and
  records ONE `StartupSample { started, startup_ms, args_fingerprint }` per
  session into `<instance>/startup_stats.json` (capped 200, atomic).
  `compute_stats(samples, current_fp)` → current-group avg + the most
  recently used different fingerprint as "before". All local.
- Commands `suggest_jvm_args`, `startup_stats` (registered in lib.rs).
- Frontend: NEW `JvmTuneModal.tsx` — reasons list, Xmx before/after,
  token-level arg diff (red struck removed / green added), merge-note banner
  when custom args exist; Apply fills jvmArgs+memory into the settings form
  + toast "review and press Save". Settings tab gained a "Recommended
  settings" (Gauge) button beside the Aikar preset and a
  `StartupStatsLine` under the args field ("avg startup before/after (n
  sessions)"). Strings under `jvm.*`.
- Tests: required-major mapping, GC flag selection per major (incl. 24+
  obsolescence), heap tiers + RAM caps, merge (preserve -D/--add-opens,
  drop old GC/Xmx, dedupe), meminfo parsing, fingerprint normalization,
  marker matching on real log-line shapes, before/after stat splits and
  missing-group handling.

## Known follow-ups / done-ness notes

- ALL FIVE PHASES ARE IMPLEMENTED. Remaining work is validation against the
  live APIs / real servers / real game launches (blocked in this sandbox)
  and the planned Norwegian locale for `src/lib/strings.ts`.
- `crosssource/mod.rs` still has a module-level `#![allow(dead_code)]`
  (a few API surfaces like `cached()`/`CrossRef` have no consumer yet).
- PUSH: `git push` AND the GitHub MCP integration both get 403 on this repo
  (App/installation has read-only access; the remote branch
  `claude/ezmapa-feature-planning-lfaqgt` was deleted at some point). All
  work exists as local commits; patch files were sent to the user as backup.
  Fix repo write access, then `git push -u origin
  claude/ezmapa-feature-planning-lfaqgt`.

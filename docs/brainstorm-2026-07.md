# EZMapa Feature Brainstorm — July 2026

Output of a structured brainstorm session (diverge → provoke → converge) for
new features beyond the current [roadmap](../ROADMAP.md). Effort scale:
S ≈ under a week, M ≈ 1–3 weeks, L ≈ a month+ of solo part-time work.

## Session frame (assumptions)

- **Goal:** differentiating, shippable features for the next 2–3 releases (v0.3.x–0.4.x), excluding the known roadmap.
- **Hard constraints assumed:** solo dev; **no hosted backend** (no server costs, no accounts DB — consistent with no-telemetry stance); Windows-first; Tauri/Rust/React stack; distribution via GitHub releases.
- **Target user assumed:** modded-Minecraft players aged ~13–30 who find Prism intimidating and the CurseForge app heavy, often playing in small friend groups.
- **Strategic thesis:** EZMapa wins on *friendly UX + dual ecosystems + smart diagnostics*. New features should deepen that moat or fill the observed social/sharing gap — without violating the no-backend constraint.
- **Success looks like:** features that (a) get used every session, (b) make someone switch launchers, or (c) make a user pull a friend in.

---

## 1. Ranked shortlist (top 7)

### 1. Quick Play + Desktop Shortcuts — effort: S
**Pitch:** Launch straight into a specific world or server — from the instance card, the command palette, or an `ezmapa://` desktop/Stream Deck shortcut. Minecraft 1.20+ supports `--quickPlaySingleplayer` / `--quickPlayMultiplayer` natively; older versions get `--server`/`--port` for multiplayer. "Play where I left off" becomes one click instead of launcher → menu → world list.
**Niche fit:** Pure friendly-UX play; no other launcher makes "resume my world" a first-class action. Daily-touch feature with the best impact-to-effort ratio on the board.
**Builds on:** `src-tauri/src/launch.rs` (argument assembly, already event-driven), `list_worlds`/`list_servers` in `src-tauri/src/commands.rs`, `src/components/CommandPalette.tsx`, plus the Tauri 2 deep-link plugin for `ezmapa://` URLs.

### 2. Pass the Pack (instance share codes) — effort: M
**Pitch:** Export any instance as a compact share code or tiny `.ezmapa` file — a manifest of loader, MC version, content list (Modrinth/CF project+version IDs), settings subset, and optionally the server list. A friend pastes the code and EZMapa reconstructs the instance by re-downloading everything from the source APIs. No file hosting, no backend — the manifest is the pack.
**Niche fit:** Directly fills the observed social gap with zero infrastructure, and it is a viral loop: every shared code is an EZMapa install prompt. Dual-ecosystem support makes this *harder for Modrinth App and CurseForge to copy* — they would only reconstruct their own half.
**Builds on:** `src/lib/export.ts` + `export_mrpack` (manifest generation already exists), `PackSource` in `src-tauri/src/models.rs`, `install_content_version`/`install_curseforge_file` for reconstruction, `SavedServer` in `src-tauri/src/servers.rs`. Known risk: some CurseForge projects disallow third-party API downloads — the manifest needs a "manual download" fallback list, mirroring how CF installs are handled today.

### 3. Preflight Check — effort: S/M
**Pitch:** A 2-second pre-launch scan that catches crashes *before* they happen: Java version mismatched to MC version, RAM allocation nonsense (too low, or exceeding physical RAM), missing mod dependencies, known-incompatible mod pairs, loader/mod version drift. Warnings show inline on the launch button with one-click fixes.
**Niche fit:** Inverts the existing crash analyzer from reactive to proactive — the cheapest possible deepening of the "smart diagnostics" moat, and a demo-able differentiator ("EZMapa stops crashes before launch").
**Builds on:** rule patterns in `src/lib/crash.ts` (many rules can be checked statically), `scan_mod_conflicts` + `system_memory_mb` in `commands.rs`, `src-tauri/src/java.rs` detection, dependency metadata already fetched in `modrinth.rs`/`curseforge.rs`.

### 4. Pack Doctor (auto-bisect crash isolation) — effort: M/L
**Pitch:** When a crash log matches no known rule, offer "Find the broken mod": EZMapa snapshots the instance, disables half the mods, relaunches, watches the exit, and binary-searches to the culprit in log2(n) rounds — then restores the snapshot and proposes a fix (update, remove, or report). Turns the single worst modded-Minecraft experience (an opaque crash in a 200-mod pack) into a supervised 5-minute procedure.
**Niche fit:** This is the flagship version of "crash analysis becomes guided troubleshooting." Nobody in the competitive set does this. It is also the feature reviewers and YouTubers would show.
**Builds on:** Surprisingly much already exists: `create_snapshot`/`restore_snapshot` (safe state), `set_mod_enabled` (toggling), `launch.rs` exit/crash events (`instance://log`, exit emission), `analyzeCrash()` in `src/lib/crash.ts` as the entry point. The new work is the orchestration state machine and wizard UI — effort L only if fully automated; ship M as a guided stepper first.

### 5. Turbo Button (one-click performance preset) — effort: S
**Pitch:** One button on any instance: "Make it faster." Installs the community-standard performance stack matched to loader + game version (Sodium/Lithium/FerriteCore-class mods on Fabric, Embeddium-class on Forge), using the existing dependency-resolving installer. Undo restores the previous state.
**Niche fit:** "My FPS is bad" is the second most common newbie pain after crashes; competitors make users research mod names. Pairs naturally with Preflight ("low RAM + no performance mods detected → offer Turbo").
**Builds on:** `install_mod`/`install_content` with auto-dependency install in `commands.rs`, version-matching logic in `modrinth.rs`. Maintenance risk: the curated slug list needs occasional updates per MC version — mitigate by driving it from Modrinth's "performance" category filtered by loader/version rather than a hardcoded list.

### 6. Loadouts (mod profiles within an instance) — effort: M
**Pitch:** Named enable/disable sets inside one instance — "Performance," "Building," "Exploration," "Vanilla-ish for the server" — switchable from the instance page or command palette without duplicating gigabytes. Stores project-ID-keyed enabled-flags so loadouts survive mod updates.
**Niche fit:** A genuine Prism-class power feature delivered with friendly UX; keeps power users from outgrowing EZMapa without intimidating new ones (it is invisible until you create a second loadout).
**Builds on:** `set_mod_enabled`/`list_mods` in `commands.rs`, `Instance` struct in `models.rs` (add `loadouts` field), instance persistence in `instances.rs`, UI in `src/pages/InstanceDetailPage.tsx`.

### 7. EZMapa Wrapped (playtime year-in-review) — effort: S/M
**Pitch:** A shareable stats card: hours per instance, most-played worlds, longest session, mods installed this year, screenshots taken — rendered as a stylish exportable image. Annual "Wrapped" moment plus an always-on stats page.
**Niche fit:** Pure delight and free marketing — every shared card advertises the launcher. Low risk, and the data is already being recorded.
**Builds on:** `list_sessions`/`record_session_dirs` (playtime already persisted per instance in `launch.rs`), `list_screenshots`, `instance_disk_usage`, existing theming system for the card design.

**Suggested sequencing:** 1 → 3 → 5 (three small wins in one release), then 2 (the differentiator release), then 4 and 6, with 7 timed for December.

---

## 2. Full idea list with clusters and kill decisions

### Cluster A — Smart diagnostics (deepens the stated moat)
| # | Idea | Verdict |
|---|---|---|
| A1 | Preflight Check (pre-launch static analysis) | **SURVIVED → shortlist #3** |
| A2 | Pack Doctor auto-bisect | **SURVIVED → shortlist #4** |
| A3 | Guided troubleshooting wizard (multi-step remedies + relaunch-and-observe) | **MERGED into A2** — same flow, the bisect is the wizard's power move |
| A4 | AI-assisted crash explanation (LLM on the log) | **KILLED** — ongoing API cost or BYO-key friction for a solo free launcher; conflicts with the no-telemetry/privacy stance; rule-based + bisect covers 90% of value |
| A5 | "What changed since you last played" mod changelog digest | **DEFERRED** — good, but belongs inside the already-planned mod update review flow; folding it in avoids double UI |

### Cluster B — Sharing without a backend (fills the social gap)
| # | Idea | Verdict |
|---|---|---|
| B1 | Instance share codes / `.ezmapa` manifest files | **SURVIVED → shortlist #2** |
| B2 | "Join my server" setup sync (server entry + required mods in one code) | **MERGED into B1** — it is a share code with a server list included |
| B3 | Friends activity feed / "what are friends playing" | **KILLED** — requires presence backend or deep Discord integration with app-approval friction; violates no-backend constraint; revisit only if EZMapa ever grows a service layer |
| B4 | LAN world discovery/join helper | **KILLED** — the game already announces LAN worlds in-game; marginal value for real UI cost |
| B5 | Curated collections discovery (surface Modrinth collections/staff picks in-app) | **BACKLOG** — decent M-effort discovery play via existing Modrinth API client, but weaker than B1 at filling the social gap; reconsider after B1 ships |

### Cluster C — Power tools (keep users from outgrowing EZMapa)
| # | Idea | Verdict |
|---|---|---|
| C1 | Loadouts (mod profiles per instance) | **SURVIVED → shortlist #6** |
| C2 | Dependency graph view + "safe to remove?" checker | **BACKLOG** — real value, but the dependency data shines brighter inside Preflight (A1) first; standalone graph UI is polish, not pull |
| C3 | Bulk instance operations (multi-select update/export/delete) | **BACKLOG** — useful, zero differentiation; do opportunistically during an InstancesPage touch |
| C4 | Instance templates (save-as-template for new-instance modal) | **KILLED** — `duplicate_instance` plus B1 share codes cover ~90% of this; redundant surface |
| C5 | CLI + `ezmapa://` deep-link protocol | **MERGED into shortlist #1** — deep links are the delivery mechanism for Quick Play shortcuts |
| C6 | Local server admin (create/manage a server jar from an instance) | **KILLED** — a second product in disguise; enormous surface (server jars, EULA, port forwarding, console) for a solo dev; hard no for now |
| C7 | Sandboxed canary update (clone → update → test-launch → auto-rollback) | **DEFERRED** — too close to the planned mod update review flow; evaluate as its v2 once snapshots + review flow coexist |

### Cluster D — Setup magic (player QoL)
| # | Idea | Verdict |
|---|---|---|
| D1 | Quick Play into world/server | **SURVIVED → shortlist #1** |
| D2 | Turbo Button performance preset | **SURVIVED → shortlist #5** |
| D3 | Global everything-search | **KILLED** — already effectively exists (Ctrl+K command palette + installed-content search); duplicate |

### Cluster E — Delight and identity
| # | Idea | Verdict |
|---|---|---|
| E1 | EZMapa Wrapped / stats dashboard | **SURVIVED → shortlist #7** |
| E2 | Screenshot gallery upgrades (clipboard copy, Discord webhook share, montage export) | **BACKLOG** — nice S/M add-on; could ride along with E1's share-card rendering work |
| E3 | 3D animated skin preview in the skin library | **BACKLOG** — polish for an existing feature (`skin.rs`); wait for a rendering excuse (E1 card could reuse the renderer) |
| E4 | Confetti/sound on first successful modded launch | **KILLED as standalone** — a moment, not a feature; fold a tasteful version into onboarding for free |

### Provoke-phase notes that shaped the cull
- **Strongest argument against the social cluster:** "no backend" kills most of it — B1 survives *because* the content is re-downloadable from Modrinth/CF, so the manifest itself is the shareable artifact. That insight is the cluster's keeper.
- **Who would hate Pack Doctor:** users on 300-mod packs where each relaunch takes 3 minutes — bisect is ~8 relaunches. Mitigation: let the crash-rule engine pre-filter suspects (recently added/updated mods first) to cut rounds.
- **The 10x version** of share codes is a public pack registry — explicitly set aside as it needs hosting/moderation; the code/file version captures most value at ~5% of the cost.
- **Feature-parity trap check:** none of the shortlist copies a competitor headline feature; Loadouts is closest to Prism territory but reframed around friendliness.

---

## 3. Assumptions to confirm

1. **No hosted backend is acceptable for v0.x** — inferred from solo dev, no monetization, no telemetry. This single assumption killed B3/C6 and shaped B1. If wrong, the ranking changes materially.
2. **Growth motive matters** — shareable/viral features (B1, E1) were weighted above what pure per-user utility would justify, assuming the goal is more users, not just happier ones.
3. **Effort calibration** — S ≈ under a week, M ≈ 1–3 weeks, L ≈ a month+ of solo part-time work, judged against the observed codebase maturity.
4. **CurseForge ToS risk on share codes** is manageable the same way current CF installs are handled (API key + manual-download fallback for opt-out projects) — worth verifying before committing to B1.
5. **Quick Play flags** (`--quickPlaySingleplayer`/`--quickPlayMultiplayer`) are available for MC 1.20+; older versions degrade gracefully to server-only quick join. Verify exact version gate during implementation.
6. **Audience skews small friend groups** — justifies ranking sharing (B1) at #2 over the more inward-facing power tools.

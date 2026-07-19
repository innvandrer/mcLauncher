## EZMapa 0.3.5 — Command Center

This is the launcher’s largest usability update so far, bringing everyday play,
pack maintenance, recovery, and sharing into one connected interface.

### A faster, more personal launcher

- New **instance Overview** with health, disk usage, startup performance,
  backups, recent activity, and one-click Smart Quick Play.
- **Launch profiles** combine a loadout, memory/JVM configuration, and optional
  world or server destination.
- The instance library now supports grid/list layouts, sorting, tags, archive
  mode, drag-to-group, multi-select, and bulk group/archive/delete actions.
- A fuzzy **Command Palette v2** searches tags, groups, and launch profiles.
- System theme, compact density, collapsible navigation, reduced transparency,
  and high-contrast modes have been added.
- Instance creation can optionally include clearly marked beta builds for
  Fabric, Quilt, Forge, and NeoForge while keeping stable versions the default.

### Activity, recovery, and diagnostics

- A persistent **Activity Center** keeps tasks, updates, backups, errors, and
  recovery events available after notifications disappear.
- **Pack Doctor v2** is a resumable multi-pass isolation workflow that narrows
  a crash to one likely mod while preserving the original loadout.
- Mod detail drawers expose source identity, dependency/removal safety, and
  recorded dependents.
- World Hub cards now show size and backup readiness; the Health Center includes
  a change/recovery timeline and transactional rollback.
- Offline readiness checks verify cached version files, assets, libraries, and
  Java before travel.

### Sharing and media

- **Pass the Pack preview** explains what will download automatically and flags
  local files that need manual sharing.
- Compact clipboard share codes can recreate an instance without a hosted
  backend or a large archive.
- **Screenshot Studio** adds an in-launcher viewer, brightness/contrast preview,
  favorites, clipboard copy, and external viewing.
- A progress-based first-launch checklist guides account setup, instance
  creation/import, and keyboard navigation.

## EZMapa 0.4.2

This release combines the v0.4.0, v0.4.1, and v0.4.2 milestones into one major
diagnostics, sharing, and recovery update.

### v0.4.0 — Health and safe updates

- **Instance Health Center** combines preflight warnings, available content
  updates, disk usage, and world-backup status in one place.
- **Reviewed updates** let you select exactly which mods, packs, and shaders to
  update.
- **Transactional content updates** back up replaced files before applying a
  reviewed batch. Failed batches restore automatically.
- **One-click rollback** restores the most recent content-update transaction
  from the Health Center.

### v0.4.1 — Pass the Pack

- Export a tiny **`.ezmapa` share manifest** containing the instance version,
  loader, content provider identities, and enabled state.
- Drop a `.ezmapa` file onto EZMapa to reconstruct the instance by downloading
  its content from Modrinth and CurseForge.
- Share manifests avoid uploading large archives and continue to use the
  launcher's existing blocked-download handling.

### v0.4.2 — Safer modding and Pack Doctor

- **Dependency-aware removal** records required dependency relationships for new
  installs and warns before removing a mod another installed project needs.
- **Pack Doctor** saves the current mod state and performs a reversible
  half-disable isolation pass for crashes the normal analyzer cannot identify.
- The original mod state can be restored from the Pack Doctor dialog at any time.

/**
 * Minimal string table for user-facing UI text, keyed so a Norwegian locale
 * can be added later without touching components. New features route their
 * strings through `t()`; existing hardcoded strings migrate opportunistically.
 *
 * Interpolation: `t("key", { n: 3 })` replaces `{n}` in the string.
 */

const en = {
  // Cross-source fallback (blocked CurseForge downloads)
  "install.viaModrinthBadge": "{n} via Modrinth",
  "install.viaModrinthToast":
    "{n} blocked CurseForge {files} auto-resolved via Modrinth (hash-verified)",
  "install.viaModrinthSingle":
    "CurseForge blocks this download — installed the identical file from Modrinth instead.",
  "blocked.title": "Manual downloads needed",
  "blocked.summary":
    "{n} {mods} must be downloaded manually: the {authors} disabled third-party downloads and no identical file exists on Modrinth.",
  "blocked.instruction":
    "Download each file from its CurseForge page and drop it into the instance's mods folder.",
  "blocked.openPage": "Open CurseForge page",
  "blocked.openFolder": "Open mods folder",
  "blocked.dismiss": "Got it",
  "blocked.mod": "mod",
  "blocked.mods": "mods",
  "blocked.file": "file",
  "blocked.files": "files",
  "blocked.author": "author",
  "blocked.authors": "authors",

  // Dual-format modpack export
  "export.cfpack": "Export as CurseForge pack",
  "export.cfpackHint": "manifest.json pack for CurseForge launchers",
  "export.both": "Export both formats",
  "export.bothHint": ".mrpack + CurseForge pack in one go",
  "export.reviewTitle": "Review platform-exclusive files",
  "export.reviewIntro":
    "The exported pack can't reference these files (they're missing from the target platform). Tick the ones to embed in the pack's overrides folder — unticked files are left out of the export.",
  "export.licenseWarning":
    "Embedding third-party mods redistributes their files. Only embed mods whose license allows redistribution — you are responsible for checking.",
  "export.embedAll": "Embed all",
  "export.missingOnCurseforge": "Not on CurseForge",
  "export.missingOnModrinth": "Not on Modrinth",
  "export.missingEverywhere": "Not on either platform",
  "export.confirm": "Export",
  "export.cancel": "Cancel",
  "export.doneMrpack": "Exported .mrpack",
  "export.doneCfpack": "Exported CurseForge pack",
  "export.doneBoth": "Exported both formats",
  "export.exporting": "Exporting modpack…",

  // "Join server → build matching instance"
  "server.buildInstance": "Create matching instance",
  "server.bestEffort":
    "Best effort: server-only mods are skipped, and client-only mods (minimaps, HUDs…) can't be detected from the server — the result may still need tweaks before it matches perfectly.",
  "server.analyzing": "Decoding the server's mod list and matching mods…",
  "server.planSummary": "{resolved} of {total} mods matched · {unresolved} unresolved · {skipped} skipped",
  "server.resolvedTitle": "Will be installed",
  "server.unresolvedTitle": "Needs manual download",
  "server.skippedTitle": "Skipped (platform / server-side only)",
  "server.approx": "closest version",
  "server.truncated": "The server truncated its mod list — some mods may be missing from this plan.",
  "server.nameLabel": "Instance name",
  "server.create": "Create instance",
  "server.created": "Instance created — installing mods in the background.",
  "server.searchLink": "Search",

  // JVM auto-tuning + startup measurement
  "jvm.recommend": "Recommended settings",
  "jvm.title": "Recommended JVM settings",
  "jvm.memory": "Memory (Xmx)",
  "jvm.argsDiff": "JVM arguments",
  "jvm.mergeNote":
    "This instance has custom JVM arguments. Only GC/heap flags are replaced — your other arguments are kept (merge shown below).",
  "jvm.apply": "Use these settings",
  "jvm.applied": "Suggestion filled in — review the fields and press Save.",
  "jvm.currentLabel": "Current",
  "jvm.suggestedLabel": "Suggested",
  "jvm.startupBoth":
    "Avg startup — before: {before}s ({nBefore} sessions) · after: {after}s ({nAfter} sessions)",
  "jvm.startupCurrentOnly": "Avg startup: {after}s ({nAfter} sessions)",
  "jvm.startupPreviousOnly":
    "Avg startup before the settings change: {before}s ({nBefore} sessions) — launch to measure the new settings.",

  // Unified update checking
  "updates.sourceModrinth": "Modrinth",
  "updates.sourceCurseforge": "CurseForge",
  "updates.switchToModrinth": "Switch source of truth to Modrinth",
  "updates.switchToCurseforge": "Switch source of truth to CurseForge",
  "updates.switched": "{file} now tracks {provider} for updates",
  "updates.fromOtherPlatform": "Newer version found on {provider}",
} as const;

export type StringKey = keyof typeof en;

type Vars = Record<string, string | number>;

/** Active locale table. Swap (or merge) with a `no` table when it lands. */
const table: Record<StringKey, string> = en;

export function t(key: StringKey, vars?: Vars): string {
  let out: string = table[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      out = out.split(`{${k}}`).join(String(v));
    }
  }
  return out;
}

/** Pick the singular/plural key for a count (English rules for now). */
export function plural(n: number, singular: StringKey, pluralKey: StringKey): string {
  return t(n === 1 ? singular : pluralKey);
}

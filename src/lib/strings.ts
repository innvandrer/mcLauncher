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

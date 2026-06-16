import type { LogLine } from "./types";

export interface CrashFinding {
  title: string;
  detail: string;
  /** Optional one-click remedy label; handled by the caller. */
  action?: "increase-ram" | "open-settings";
}

interface Rule {
  test: RegExp;
  title: string;
  detail: string;
  action?: CrashFinding["action"];
}

// Ordered by specificity — the first matching rule wins so we surface the most
// actionable cause rather than a generic stack trace.
const RULES: Rule[] = [
  {
    test: /java\.lang\.OutOfMemoryError|GC overhead limit|Out of memory/i,
    title: "Ran out of memory",
    detail:
      "The game hit its RAM ceiling. Modded packs often need 6–8 GB. Raise the memory allocation for this instance and try again.",
    action: "increase-ram",
  },
  {
    test: /UnsupportedClassVersionError|compiled by a more recent version of the Java/i,
    title: "Wrong Java version",
    detail:
      "A mod needs a newer Java than the one launching this instance. Modern Minecraft wants Java 17 or 21 — set a matching Java path in settings.",
    action: "open-settings",
  },
  {
    test: /Missing or unsupported mandatory dependencies|requires (version|any version of)|Incompatible mod set|missing dependenc/i,
    title: "Missing mod dependency",
    detail:
      "A mod is missing a required library (e.g. Fabric API, a core lib, or a specific version). Check the lines below for the named mod and install it.",
  },
  {
    test: /found a duplicate mod|Duplicate mods|Found duplicate mods/i,
    title: "Duplicate mods",
    detail:
      "Two copies of the same mod are installed. Remove the older duplicate from the mods folder.",
  },
  {
    test: /Mixin apply failed|Mixin transformation of|org\.spongepowered\.asm\.mixin/i,
    title: "Mixin / mod conflict",
    detail:
      "A mod's mixin failed to apply — usually a version mismatch or two incompatible mods. The mod named just before this error is the likely culprit.",
  },
  {
    test: /EXCEPTION_ACCESS_VIOLATION|Pixel format not accelerated|Couldn't set pixel format|Failed to create window|OpenGL/i,
    title: "Graphics / driver issue",
    detail:
      "The crash points at the GPU or OpenGL. Update your graphics drivers; if you use a shader pack, try disabling it.",
  },
  {
    test: /NoClassDefFoundError|ClassNotFoundException/i,
    title: "Missing class (broken mod)",
    detail:
      "A mod referenced code that isn't present — often a missing dependency or a mod built for a different Minecraft version.",
  },
];

/**
 * Scan recent log output for a known crash signature. Returns the most specific
 * finding, or null when nothing recognizable is present.
 */
export function analyzeCrash(logs: LogLine[]): CrashFinding | null {
  if (logs.length === 0) return null;
  // Only consider the tail — crashes surface near the end of the log.
  const text = logs
    .slice(-400)
    .map((l) => l.line)
    .join("\n");

  // Require some signal that a crash actually happened to avoid false positives
  // on healthy logs that merely mention "OpenGL" etc.
  const looksLikeCrash =
    /Exception|Error|crash|fatal|failed|OutOfMemory/i.test(text);
  if (!looksLikeCrash) return null;

  for (const rule of RULES) {
    if (rule.test.test(text)) {
      return { title: rule.title, detail: rule.detail, action: rule.action };
    }
  }
  return null;
}

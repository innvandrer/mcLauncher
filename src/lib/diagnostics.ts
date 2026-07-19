import type { ModEntry } from "./types";

export function healthScore(warningCount: number, updateCount: number): number {
  return Math.max(0, 100 - warningCount * 20 - Math.min(updateCount, 5) * 4);
}

export function firstDoctorSuspects(mods: ModEntry[]): ModEntry[] {
  return mods
    .filter((mod) => mod.enabled)
    .filter((_, index) => index % 2 === 0);
}

import { describe, expect, it } from "vitest";
import { firstDoctorSuspects, healthScore } from "./diagnostics";
import type { ModEntry } from "./types";

describe("healthScore", () => {
  it("penalizes warnings more heavily than routine updates", () => {
    expect(healthScore(1, 0)).toBe(80);
    expect(healthScore(0, 1)).toBe(96);
  });

  it("never falls below zero", () => {
    expect(healthScore(99, 99)).toBe(0);
  });
});

describe("firstDoctorSuspects", () => {
  it("selects half of enabled mods and ignores disabled files", () => {
    const mods: ModEntry[] = [
      { fileName: "a.jar", enabled: true, size: 1 },
      { fileName: "b.jar", enabled: false, size: 1 },
      { fileName: "c.jar", enabled: true, size: 1 },
      { fileName: "d.jar", enabled: true, size: 1 },
    ];
    expect(firstDoctorSuspects(mods).map((mod) => mod.fileName)).toEqual([
      "a.jar",
      "d.jar",
    ]);
  });
});

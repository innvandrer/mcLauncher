import { describe, expect, it } from "vitest";
import { analyzeCrash } from "./crash";
import type { LogLine } from "./types";

const lines = (...text: string[]): LogLine[] =>
  text.map((line) => ({ instanceId: "test", line, isErr: true }));

describe("analyzeCrash", () => {
  it("returns the actionable out-of-memory finding", () => {
    expect(
      analyzeCrash(lines("java.lang.OutOfMemoryError: Java heap space")),
    ).toMatchObject({
      title: "Ran out of memory",
      action: "increase-ram",
    });
  });

  it("does not flag healthy graphics initialization as a crash", () => {
    expect(
      analyzeCrash(lines("OpenGL renderer initialized successfully")),
    ).toBeNull();
  });

  it("recognizes missing dependency failures", () => {
    expect(
      analyzeCrash(
        lines("Fatal error: Missing or unsupported mandatory dependencies"),
      ),
    ).toMatchObject({ title: "Missing mod dependency" });
  });
});

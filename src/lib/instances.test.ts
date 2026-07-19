import { describe, expect, it } from "vitest";
import { groupInstances, selectContinueInstance } from "./instances";
import type { Instance } from "./types";

function instance(overrides: Partial<Instance>): Instance {
  return {
    id: overrides.id ?? "id",
    name: overrides.name ?? "Instance",
    mcVersion: overrides.mcVersion ?? "1.21.1",
    loader: overrides.loader ?? "fabric",
    created: overrides.created ?? 0,
    totalPlaySeconds: overrides.totalPlaySeconds ?? 0,
    ...overrides,
  };
}

describe("selectContinueInstance", () => {
  it("prefers the most recently played instance over the most played one", () => {
    const selected = selectContinueInstance([
      instance({ id: "long", lastPlayed: 100, totalPlaySeconds: 100_000 }),
      instance({ id: "recent", lastPlayed: 200, totalPlaySeconds: 60 }),
    ]);
    expect(selected?.id).toBe("recent");
  });

  it("falls back to playtime when no instance has a recent timestamp", () => {
    const selected = selectContinueInstance([
      instance({ id: "a", totalPlaySeconds: 20 }),
      instance({ id: "b", totalPlaySeconds: 40 }),
    ]);
    expect(selected?.id).toBe("b");
  });
});

describe("groupInstances", () => {
  it("sorts named groups and places ungrouped instances last", () => {
    const groups = groupInstances([
      instance({ id: "plain", name: "Plain" }),
      instance({ id: "tech", name: "Tech", group: "Friends" }),
      instance({ id: "solo", name: "Solo", group: "Solo" }),
    ]);
    expect(groups.map((group) => group.label)).toEqual(["Friends", "Solo", ""]);
  });

  it("matches group names during search", () => {
    const groups = groupInstances(
      [
        instance({ id: "one", name: "Pack One", group: "EZMapa crew" }),
        instance({ id: "two", name: "Pack Two", group: "Solo" }),
      ],
      "crew",
    );
    expect(
      groups.flatMap((group) => group.instances).map((item) => item.id),
    ).toEqual(["one"]);
  });
});

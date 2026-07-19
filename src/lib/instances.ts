import type { Instance, Loader } from "./types";
import { loaderLabel } from "./utils";

export type LoaderFilter = Loader | "all";

export interface InstanceGroup {
  key: string;
  label: string;
  instances: Instance[];
}

export function sortInstances(instances: Instance[]): Instance[] {
  return [...instances].sort((a, b) => {
    if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
    const aPlayed = a.lastPlayed ?? 0;
    const bPlayed = b.lastPlayed ?? 0;
    if (aPlayed !== bPlayed) return bPlayed - aPlayed;
    return a.name.localeCompare(b.name);
  });
}

export function groupInstances(
  instances: Instance[],
  query = "",
  loaderFilter: LoaderFilter = "all",
): InstanceGroup[] {
  const q = query.trim().toLowerCase();
  const filtered = instances.filter((instance) => {
    if (loaderFilter !== "all" && instance.loader !== loaderFilter)
      return false;
    if (!q) return true;
    return (
      instance.name.toLowerCase().includes(q) ||
      instance.mcVersion.toLowerCase().includes(q) ||
      loaderLabel(instance.loader).toLowerCase().includes(q) ||
      instance.group?.toLowerCase().includes(q) ||
      instance.tags?.some((tag) => tag.toLowerCase().includes(q))
    );
  });

  const grouped = new Map<string, Instance[]>();
  for (const instance of filtered) {
    const key = instance.group?.trim() || "";
    const bucket = grouped.get(key) ?? [];
    bucket.push(instance);
    grouped.set(key, bucket);
  }

  return [...grouped.entries()]
    .sort(([a], [b]) => {
      if (!a) return 1;
      if (!b) return -1;
      return a.localeCompare(b);
    })
    .map(([key, items]) => ({
      key: key || "__ungrouped",
      label: key,
      instances: sortInstances(items),
    }));
}

export function selectContinueInstance(instances: Instance[]): Instance | null {
  if (instances.length === 0) return null;
  return [...instances].sort((a, b) => {
    const recent = (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0);
    if (recent !== 0) return recent;
    const playtime = (b.totalPlaySeconds ?? 0) - (a.totalPlaySeconds ?? 0);
    if (playtime !== 0) return playtime;
    return a.name.localeCompare(b.name);
  })[0];
}

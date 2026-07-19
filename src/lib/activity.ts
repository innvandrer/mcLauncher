import { useSyncExternalStore } from "react";

export type ActivityKind = "launch" | "content" | "backup" | "crash" | "system";

export interface ActivityEntry {
  id: string;
  created: number;
  kind: ActivityKind;
  message: string;
  detail?: string;
  instanceId?: string;
}

const STORAGE_KEY = "ezmapa:activity:v1";
const listeners = new Set<() => void>();
let entries: ActivityEntry[] = load();

function load(): ActivityEntry[] {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
  } catch {
    return [];
  }
}

function commit(next: ActivityEntry[]) {
  entries = next.slice(0, 250);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // Activity history is helpful, but must never block launcher actions.
  }
  listeners.forEach((listener) => listener());
}

export function recordActivity(entry: Omit<ActivityEntry, "id" | "created">) {
  commit([
    {
      ...entry,
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      created: Date.now(),
    },
    ...entries,
  ]);
}

export function clearActivities() {
  commit([]);
}

export function useActivities() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => entries,
    () => entries,
  );
}

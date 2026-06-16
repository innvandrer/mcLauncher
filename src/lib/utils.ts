import { clsx, type ClassValue } from "clsx";

export function cn(...inputs: ClassValue[]): string {
  return clsx(inputs);
}

export function formatPlaytime(seconds: number): string {
  if (!seconds) return "Never played";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m played`;
  if (m > 0) return `${m}m played`;
  return `${seconds}s played`;
}

/** Compact duration like "12h 34m", "34m" or "0m" — no trailing "played". */
export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function timeAgo(unixSeconds?: number | null): string {
  if (!unixSeconds) return "never";
  const diff = Date.now() / 1000 - unixSeconds;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(unixSeconds * 1000).toLocaleDateString();
}

/** True when an instance icon is an image (a URL or data URI) rather than an
 *  emoji / single-letter string. Modpack instances store their pack art as a
 *  remote URL, while user-set icons are emoji. */
export function isImageIcon(icon?: string | null): boolean {
  return !!icon && /^(https?:\/\/|data:image\/)/i.test(icon);
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

export function formatBytes(n: number): string {
  if (n >= 1_048_576) return `${(n / 1_048_576).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${n} B`;
}

/** Bytes/second formatted as a transfer rate, e.g. "12.4 MB/s". */
export function formatSpeed(bytesPerSec?: number | null): string {
  if (!bytesPerSec || bytesPerSec <= 0) return "";
  return `${formatBytes(bytesPerSec)}/s`;
}

/** Rough ETA from a started timestamp and file-count progress. */
export function formatEta(startedMs: number, current: number, total: number): string {
  if (current <= 0 || total <= 0 || current >= total) return "";
  const elapsed = (Date.now() - startedMs) / 1000;
  if (elapsed < 0.5) return "";
  const remaining = (elapsed / current) * (total - current);
  if (remaining < 1) return "";
  if (remaining < 60) return `${Math.ceil(remaining)}s left`;
  const m = Math.floor(remaining / 60);
  const s = Math.round(remaining % 60);
  return `${m}m ${s}s left`;
}

export const ACCENTS: Record<string, string> = {
  violet: "263 85% 66%",
  blue: "217 91% 60%",
  emerald: "152 69% 46%",
  rose: "346 84% 61%",
  amber: "38 92% 55%",
  cyan: "189 85% 52%",
};

export function applyTheme(theme: string, accent: string) {
  const root = document.documentElement;
  if (theme === "light") root.classList.remove("dark");
  else root.classList.add("dark");
  const hsl = ACCENTS[accent] ?? ACCENTS.violet;
  root.style.setProperty("--accent", hsl);
  root.style.setProperty("--ring", hsl);
}

export const LOADERS: { id: Loader; label: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "quilt", label: "Quilt" },
  { id: "forge", label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
];

import type { Loader } from "./types";

export function loaderLabel(loader: Loader): string {
  return LOADERS.find((l) => l.id === loader)?.label ?? loader;
}

import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  ArrowLeft,
  Camera,
  Check,
  ChevronLeft,
  ChevronRight,
  Download,
  ExternalLink,
  FolderOpen,
  Globe,
  Image,
  Package,
  Play,
  RefreshCw,
  Save,
  ScrollText,
  Search,
  Settings2,
  Sparkles,
  Square,
  Trash2,
  Upload,
  Server,
  Users,
  Wifi,
  Copy,
} from "lucide-react";
import { Button, EmptyState, Field, Input, Modal, Select, Skeleton, Spinner } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import { ACCENTS, AIKAR_FLAGS, cn, formatBytes, formatNumber, isImageIcon, loaderLabel } from "@/lib/utils";
import { InstanceIcon } from "@/components/InstanceIcon";
import { LoaderLogo } from "@/components/LoaderLogo";
import { ExportMenu } from "@/components/ExportMenu";
import { LoadoutsMenu } from "@/components/LoadoutsMenu";
import { analyzeCrash, type CrashFinding } from "@/lib/crash";
import { exportInstanceMrpack } from "@/lib/export";
import type {
  ContentVersion,
  DiskUsage,
  InstallOutcome,
  Instance,
  ModConflict,
  ModEntry,
  ModHit,
  ModpackUpdate,
  ModUpdate,
  ResourcePackEntry,
  SavedServer,
  ScreenshotEntry,
  ServerStatus,
  ShaderEntry,
  Snapshot,
  WorldEntry,
} from "@/lib/types";

type Tab = "content" | "servers" | "logs" | "settings";

function contentUrl(hit: ModHit, provider: Provider): string {
  const typeSlug: Record<string, string> = {
    mod: provider === "curseforge" ? "mc-mods" : "mod",
    resourcepack: provider === "curseforge" ? "texture-packs" : "resourcepack",
    shader: provider === "curseforge" ? "customization" : "shader",
    modpack: provider === "curseforge" ? "modpacks" : "modpack",
  };
  const section = typeSlug[hit.project_type] ?? (provider === "curseforge" ? "mc-mods" : "mod");
  if (provider === "curseforge") {
    return `https://www.curseforge.com/minecraft/${section}/${hit.slug}`;
  }
  return `https://modrinth.com/${section}/${hit.slug}`;
}

export function InstanceDetailPage({ id }: { id: string }) {
  const instance = useStore((s) => s.instances.find((i) => i.id === id));
  const running = useStore((s) => s.running.has(id));
  const launch = useStore((s) => s.launch);
  const stop = useStore((s) => s.stop);
  const close = useStore((s) => s.closeInstance);
  const toast = useStore((s) => s.toast);
  const [tab, setTab] = useState<Tab>("content");

  if (!instance) {
    return (
      <div className="flex h-full items-center justify-center">
        <Button variant="ghost" onClick={close}>
          <ArrowLeft className="h-4 w-4" /> Back
        </Button>
      </div>
    );
  }

  const tabs: { id: Tab; label: string; icon: typeof Package }[] = [
    { id: "content", label: "Content", icon: Package },
    { id: "servers", label: "Servers", icon: Server },
    { id: "logs", label: "Logs", icon: ScrollText },
    { id: "settings", label: "Settings", icon: Settings2 },
  ];

  return (
    <div className="app-page">
      {/* Header */}
      <header className="app-gutter shrink-0 pt-6">
        <button
          onClick={close}
          className="mb-4 inline-flex items-center gap-1.5 text-sm text-muted-foreground transition hover:text-foreground btn-focus"
        >
          <ArrowLeft className="h-4 w-4" /> All instances
        </button>

        <div className="flex flex-wrap items-start gap-4">
          <InstanceIcon instance={instance} size="detail" className="shrink-0 border border-white/5" />
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-2xl font-bold tracking-tight">{instance.name}</h1>
            <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-sm text-muted-foreground">
              <span>{instance.mcVersion}</span>
              {instance.loader !== "vanilla" && (
                <>
                  <span className="opacity-40">•</span>
                  <span className="font-medium text-accent">{loaderLabel(instance.loader)}</span>
                  {instance.loaderVersion && <span>{instance.loaderVersion}</span>}
                </>
              )}
            </div>
          </div>
          <div className="flex w-full flex-wrap items-center gap-2 sm:w-auto sm:justify-end">
            <ExportMenu instanceId={id} instanceName={instance.name} />
            <Button variant="secondary" onClick={() => api.openInstanceFolder(id)}>
              <FolderOpen className="h-4 w-4" /> Folder
            </Button>
            {running ? (
              <Button variant="danger" onClick={() => stop(id)}>
                <Square className="h-4 w-4 fill-current" /> Stop
              </Button>
            ) : (
              <Button variant="primary" onClick={() => launch(id)}>
                <Play className="h-4 w-4 fill-current" /> Play
              </Button>
            )}
          </div>
        </div>

        {/* Tabs */}
        <nav className="mt-6 flex gap-1 overflow-x-auto border-b border-border">
          {tabs.map((t) => {
            const Icon = t.icon;
            const active = tab === t.id;
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={cn(
                  "relative flex shrink-0 items-center gap-2 px-4 py-2.5 text-sm font-medium transition btn-focus",
                  active ? "text-foreground" : "text-muted-foreground hover:text-foreground",
                )}
              >
                <Icon className="h-4 w-4" />
                {t.label}
                {active && (
                  <span className="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-accent" />
                )}
              </button>
            );
          })}
        </nav>
      </header>

      <div className="app-gutter shrink-0">
        <ModpackUpdateBanner instance={instance} />
      </div>

      <div className="app-scroll app-gutter py-5">
        {tab === "content" && <ContentTab instance={instance} />}
        {tab === "servers" && <ServersTab instance={instance} />}
        {tab === "logs" && (
          <LogsTab id={id} instance={instance} onOpenSettings={() => setTab("settings")} />
        )}
        {tab === "settings" && <SettingsTab instance={instance} />}
      </div>
    </div>
  );
}

// --------------------------------------------------------------------------
// Modpack update banner — checks for a newer pack version and shows the diff
// --------------------------------------------------------------------------

function ModpackUpdateBanner({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const [update, setUpdate] = useState<ModpackUpdate | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    setUpdate(null);
    setExpanded(false);
    // Only modpack-derived instances carry a pack source; the command returns
    // null for everything else, so this is a cheap no-op otherwise.
    api.checkModpackUpdate(instance.id).then(setUpdate).catch(() => {});
  }, [instance.id]);

  if (!update) return null;

  const apply = async () => {
    setApplying(true);
    toast("info", "Updating modpack…");
    try {
      await api.applyModpackUpdate(instance.id, update.versionId);
      toast("success", `Updated to ${update.versionName}`);
      setUpdate(null);
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setApplying(false);
    }
  };

  const Line = ({ label, items, color }: { label: string; items: string[]; color: string }) =>
    items.length === 0 ? null : (
      <div className="mt-2">
        <p className={cn("text-xs font-semibold", color)}>
          {label} ({items.length})
        </p>
        <ul className="mt-0.5 space-y-0.5 text-xs text-muted-foreground">
          {items.map((f) => (
            <li key={f} className="truncate">{f}</li>
          ))}
        </ul>
      </div>
    );

  return (
    <div className="mt-4 rounded-xl border border-accent/40 bg-accent/10 p-3">
      <div className="flex flex-wrap items-center gap-3">
        <Download className="h-4 w-4 shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold">
            Modpack update available — {update.versionName}
          </p>
          <p className="text-xs text-muted-foreground">
            {update.currentVersion ? `From ${update.currentVersion} · ` : ""}
            {update.added.length} added · {update.updated.length} updated ·{" "}
            {update.removed.length} removed
          </p>
        </div>
        <Button size="sm" variant="ghost" onClick={() => setExpanded((v) => !v)}>
          {expanded ? "Hide" : "View changes"}
        </Button>
        <Button size="sm" variant="primary" onClick={apply} loading={applying}>
          Update
        </Button>
      </div>
      {expanded && (
        <div className="mt-2 border-t border-accent/20 pt-1">
          <Line label="Added" items={update.added} color="text-emerald-400" />
          <Line label="Updated" items={update.updated} color="text-accent" />
          <Line label="Removed" items={update.removed} color="text-destructive" />
        </div>
      )}
    </div>
  );
}

// --------------------------------------------------------------------------
// Content tab — sub-tabs for Mods, Resource Packs, Shaders, Worlds, Screenshots
// --------------------------------------------------------------------------

type ContentSection = "mods" | "resourcepacks" | "shaders" | "worlds" | "screenshots";

function ContentTab({ instance }: { instance: Instance }) {
  const isModded = instance.loader !== "vanilla";
  const [section, setSection] = useState<ContentSection>(isModded ? "mods" : "resourcepacks");

  const subtabs: { id: ContentSection; label: string; icon: typeof Package }[] = [
    ...(isModded
      ? [{ id: "mods" as ContentSection, label: "Mods", icon: Package }]
      : []),
    { id: "resourcepacks", label: "Resource Packs", icon: Image },
    { id: "shaders", label: "Shaders", icon: Sparkles },
    { id: "worlds", label: "Worlds", icon: Globe },
    { id: "screenshots", label: "Screenshots", icon: Camera },
  ];

  return (
    <div>
      {/* Sub-tab bar */}
      <div className="mb-5 flex flex-wrap gap-1">
        {subtabs.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              onClick={() => setSection(t.id)}
              className={cn(
                "flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition btn-focus",
                section === t.id
                  ? "bg-accent/15 text-accent"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <Icon className="h-3.5 w-3.5" />
              {t.label}
            </button>
          );
        })}
      </div>

      {section === "mods" && <ModsPanel instance={instance} />}
      {section === "resourcepacks" && (
        <ContentBrowserPanel
          instance={instance}
          contentType="resourcepack"
          modrinthType="resourcepack"
          useLoaderFilter={false}
          listItems={() => api.listResourcePacks(instance.id)}
          deleteItem={(fileName) => api.deleteResourcePack(instance.id, fileName)}
          emptyLabel="resource packs"
          placeholder={`Search Modrinth for ${instance.mcVersion} resource packs…`}
        />
      )}
      {section === "shaders" && (
        <ContentBrowserPanel
          instance={instance}
          contentType="shader"
          modrinthType="shader"
          useLoaderFilter={false}
          listItems={() => api.listShaders(instance.id)}
          deleteItem={(fileName) => api.deleteShader(instance.id, fileName)}
          emptyLabel="shaders"
          placeholder={`Search Modrinth for ${instance.mcVersion} shaders…`}
        />
      )}
      {section === "worlds" && <WorldsPanel instance={instance} />}
      {section === "screenshots" && <ScreenshotsPanel instance={instance} />}
    </div>
  );
}

// --------------------------------------------------------------------------
// Content source provider (Modrinth / CurseForge)
// --------------------------------------------------------------------------

type Provider = "modrinth" | "curseforge";

function ProviderToggle({
  provider,
  onChange,
}: {
  provider: Provider;
  onChange: (p: Provider) => void;
}) {
  const opts: { id: Provider; label: string }[] = [
    { id: "modrinth", label: "Modrinth" },
    { id: "curseforge", label: "CurseForge" },
  ];
  return (
    <div className="inline-flex shrink-0 rounded-lg border bg-card/60 p-0.5">
      {opts.map((o) => (
        <button
          key={o.id}
          onClick={() => onChange(o.id)}
          className={cn(
            "rounded-md px-2.5 py-1 text-xs font-medium transition btn-focus",
            provider === o.id
              ? "bg-accent/20 text-accent"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

// Run a provider-aware search; returns the hits.
function searchProvider(
  provider: Provider,
  args: {
    query: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
    limit?: number;
    offset?: number;
  },
) {
  if (provider === "curseforge") {
    return api.searchCurseforge({
      query: args.query,
      contentType: args.contentType,
      loader: args.loader,
      gameVersion: args.gameVersion,
      limit: args.limit,
      offset: args.offset,
    });
  }
  return api.searchModrinth({
    query: args.query,
    projectType: args.contentType,
    loader: args.loader,
    gameVersion: args.gameVersion,
    limit: args.limit,
    offset: args.offset,
  });
}

// Run a provider-aware install; returns the installed file + any dependencies.
function installProvider(
  provider: Provider,
  args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  },
): Promise<InstallOutcome> {
  if (provider === "curseforge") {
    return api.installCurseforgeContent(args);
  }
  return api.installContent(args);
}

// Number of search results per page.
const PAGE_SIZE = 20;
// Cap pagination so we never request offsets the providers reject.
const MAX_PAGES = 50;

function pageCountFor(totalHits: number): number {
  return Math.min(Math.ceil(totalHits / PAGE_SIZE), MAX_PAGES);
}

/** Client-side pagination for installed/local lists (same page size as browse). */
function useLocalPagination<T>(items: T[], resetKey: string) {
  const [page, setPage] = useState(0);
  useEffect(() => {
    setPage(0);
  }, [resetKey]);
  const pageCount = pageCountFor(items.length);
  const pagedItems = useMemo(
    () => items.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE),
    [items, page],
  );
  return { page, setPage, pageCount, pagedItems };
}

function useContentUpdates(
  instance: Instance,
  contentType: string | undefined,
  onUpdated: () => void,
) {
  const toast = useStore((s) => s.toast);
  const [updates, setUpdates] = useState<ModUpdate[]>([]);
  const [checking, setChecking] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);

  const checkUpdates = async () => {
    setChecking(true);
    try {
      const all = await api.checkModUpdates({
        instanceId: instance.id,
        loader: instance.loader,
        gameVersion: instance.mcVersion,
      });
      const filtered = contentType
        ? all.filter((u) => u.contentType === contentType)
        : all;
      setUpdates(filtered);
      const label =
        contentType === "resourcepack"
          ? "resource packs"
          : contentType === "shader"
            ? "shaders"
            : "mods";
      toast(
        filtered.length ? "info" : "success",
        filtered.length
          ? `${filtered.length} ${label} update${filtered.length > 1 ? "s" : ""} available`
          : `All ${label} up to date`,
      );
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setChecking(false);
    }
  };

  const applyAllUpdates = async () => {
    setUpdatingAll(true);
    try {
      for (const u of updates) {
        await api.applyModUpdate(instance.id, u);
      }
      toast("success", `Updated ${updates.length} item${updates.length > 1 ? "s" : ""}`);
      setUpdates([]);
      onUpdated();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setUpdatingAll(false);
    }
  };

  return { updates, checking, updatingAll, checkUpdates, applyAllUpdates };
}

function ContentUpdateButton({
  checking,
  onClick,
}: {
  checking: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={checking}
      className="inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus disabled:opacity-50"
    >
      <RefreshCw className={cn("h-3 w-3", checking && "animate-spin")} />
      {checking ? "Checking…" : "Updates"}
    </button>
  );
}

function ContentUpdatesBanner({
  updates,
  updatingAll,
  onApply,
}: {
  updates: ModUpdate[];
  updatingAll: boolean;
  onApply: () => void;
}) {
  if (updates.length === 0) return null;
  return (
    <div className="mb-3 rounded-lg border border-accent/40 bg-accent/10 p-3">
      <p className="text-sm font-medium">
        {updates.length} update{updates.length > 1 ? "s" : ""} available
      </p>
      <ul className="mt-1.5 space-y-0.5 text-xs text-muted-foreground">
        {updates.slice(0, 4).map((u) => (
          <li key={u.oldFileName} className="truncate">
            {u.newFileName.replace(/\.(jar|zip)$/, "")} · v{u.versionNumber}
          </li>
        ))}
        {updates.length > 4 && <li>+{updates.length - 4} more…</li>}
      </ul>
      <Button size="sm" variant="primary" className="mt-2 w-full" loading={updatingAll} onClick={onApply}>
        Update all
      </Button>
    </div>
  );
}

// Prev / page X of Y / Next controls. Renders nothing for a single page.
function Pagination({
  page,
  pageCount,
  onPage,
}: {
  page: number;
  pageCount: number;
  onPage: (p: number) => void;
}) {
  if (pageCount <= 1) return null;
  return (
    <div className="mt-4 flex items-center justify-center gap-3">
      <Button
        size="sm"
        variant="secondary"
        disabled={page <= 0}
        onClick={() => onPage(page - 1)}
      >
        <ChevronLeft className="h-4 w-4" /> Prev
      </Button>
      <span className="text-sm text-muted-foreground">
        Page {page + 1} of {pageCount}
      </span>
      <Button
        size="sm"
        variant="secondary"
        disabled={page >= pageCount - 1}
        onClick={() => onPage(page + 1)}
      >
        Next <ChevronRight className="h-4 w-4" />
      </Button>
    </div>
  );
}

// Small icon tile for an installed item: real project icon when we have one
// (matched from current search results), otherwise a fallback glyph.
function InstalledIcon({
  iconUrl,
  fallback,
}: {
  iconUrl?: string | null;
  fallback: ReactNode;
}) {
  if (iconUrl) {
    return <img src={iconUrl} alt="" className="h-9 w-9 shrink-0 rounded-md object-cover" />;
  }
  return (
    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-muted">
      {fallback}
    </div>
  );
}

function matchesInstalledQuery(query: string, fileName: string, projectId?: string | null): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;

  const baseName = fileName.replace(/\.(jar|zip)$/, "").toLowerCase();

  return (
    fileName.toLowerCase().includes(q) ||
    baseName.includes(q) ||
    (projectId?.toLowerCase().includes(q) ?? false)
  );
}

// --------------------------------------------------------------------------
// Mods panel (has enable/disable toggle)
// --------------------------------------------------------------------------

function ModsPanel({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const refreshInstances = useStore((s) => s.refreshInstances);
  const [installed, setInstalled] = useState<ModEntry[]>([]);
  const [installedQuery, setInstalledQuery] = useState("");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [page, setPage] = useState(0);
  const [totalHits, setTotalHits] = useState(0);
  const [conflicts, setConflicts] = useState<ModConflict[]>([]);
  const [cleaning, setCleaning] = useState(false);

  const refresh = () => {
    api.listMods(instance.id).then(setInstalled).catch(() => {});
    api.scanModConflicts(instance.id).then(setConflicts).catch(() => {});
  };

  // Keep the newest version of each duplicated mod, delete the older copies.
  const cleanupConflicts = async () => {
    setCleaning(true);
    try {
      const removed = await api.resolveModConflicts(instance.id);
      toast(
        removed.length ? "success" : "info",
        removed.length
          ? `Removed ${removed.length} older ${removed.length === 1 ? "copy" : "copies"}`
          : "Nothing to clean up",
      );
      refresh();
      refreshInstances();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setCleaning(false);
    }
  };
  useEffect(() => {
    setInstalledQuery("");
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  useEffect(() => {
    setSearching(true);
    const t = setTimeout(() => {
      searchProvider(provider, {
        query,
        contentType: "mod",
        loader: instance.loader,
        gameVersion: instance.mcVersion,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      })
        .then((r) => {
          setResults(r.hits);
          setTotalHits(r.total_hits);
        })
        .catch((e) => toast("error", errMessage(e)))
        .finally(() => setSearching(false));
    }, 350);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, provider, page, instance.loader, instance.mcVersion]);

  // Map projectId → icon from the current results, so installed mods that also
  // appear in the search show their real icon.
  const iconByProjectId = new Map(
    results.filter((r) => r.icon_url).map((r) => [r.project_id, r.icon_url!]),
  );

  const filteredInstalled = useMemo(
    () => installed.filter((m) => matchesInstalledQuery(installedQuery, m.fileName, m.projectId)),
    [installed, installedQuery],
  );

  const {
    page: installedPage,
    setPage: setInstalledPage,
    pageCount: installedPageCount,
    pagedItems: pagedInstalled,
  } = useLocalPagination(filteredInstalled, `${instance.id}:${installedQuery}`);

  const [versionPickTarget, setVersionPickTarget] = useState<ModHit | null>(null);

  const installWithVersion = async (hit: ModHit, versionId: string) => {
    setInstalling(hit.project_id);
    setVersionPickTarget(null);
    try {
      const res =
        provider === "curseforge"
          ? await api.installCurseforgeFile({
              instanceId: instance.id,
              projectId: hit.project_id,
              fileId: versionId,
              contentType: "mod",
              loader: instance.loader,
              gameVersion: instance.mcVersion,
            })
          : await api.installContentVersion({
              instanceId: instance.id,
              projectId: hit.project_id,
              versionId,
              contentType: "mod",
              loader: instance.loader,
              gameVersion: instance.mcVersion,
            });
      const n = res.dependencies.length;
      toast(
        "success",
        n
          ? `Installed ${res.file} · +${n} ${n === 1 ? "dependency" : "dependencies"}`
          : `Installed ${res.file}`,
      );
      refresh();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setInstalling(null);
    }
  };

  const toggle = async (m: ModEntry) => {
    await api.setModEnabled(instance.id, m.fileName, !m.enabled).catch((e) =>
      toast("error", errMessage(e)),
    );
    refresh();
  };
  const remove = async (m: ModEntry) => {
    await api.deleteMod(instance.id, m.fileName).catch((e) =>
      toast("error", errMessage(e)),
    );
    refresh();
  };

  const { updates, checking, updatingAll, checkUpdates, applyAllUpdates } = useContentUpdates(
    instance,
    "mod",
    refresh,
  );

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      {/* Installed */}
      <section className="min-w-0">
        <div className="mb-3 flex items-center justify-between gap-2">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Installed ({filteredInstalled.length}
            {installedQuery.trim() ? ` / ${installed.length}` : ""})
          </h2>
          {installed.length > 0 && (
            <div className="flex items-center gap-3">
              <LoadoutsMenu instance={instance} onApplied={refresh} />
              <ContentUpdateButton checking={checking} onClick={checkUpdates} />
            </div>
          )}
        </div>

        {installed.length > 0 && (
          <div className="relative mb-3">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={installedQuery}
              onChange={(e) => setInstalledQuery(e.target.value)}
              placeholder="Search installed mods…"
              className="pl-9"
            />
          </div>
        )}

        {conflicts.length > 0 && (
          <div className="mb-3 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3">
            <p className="flex items-center gap-1.5 text-sm font-medium text-amber-200">
              <AlertTriangle className="h-3.5 w-3.5" />
              {conflicts.length} possible duplicate{conflicts.length > 1 ? "s" : ""}
            </p>
            <ul className="mt-1.5 space-y-1 text-xs text-muted-foreground">
              {conflicts.map((c) => (
                <li key={c.name}>
                  <span className="font-medium text-foreground/90">{c.name}</span>:{" "}
                  {c.files.join(", ")}
                </li>
              ))}
            </ul>
            <p className="mt-1.5 text-[11px] text-muted-foreground/70">
              Two builds of the same mod can crash the game. This keeps the newest
              version of each and removes the rest.
            </p>
            <Button
              size="sm"
              variant="secondary"
              className="mt-2"
              loading={cleaning}
              onClick={cleanupConflicts}
            >
              <Trash2 className="h-3.5 w-3.5" /> Remove older copies
            </Button>
          </div>
        )}

        <ContentUpdatesBanner
          updates={updates}
          updatingAll={updatingAll}
          onApply={applyAllUpdates}
        />
        {installed.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No mods yet. Search on the right to add some.
          </p>
        ) : filteredInstalled.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No installed mods match &ldquo;{installedQuery.trim()}&rdquo;.
          </p>
        ) : (
          <div className="space-y-1.5">
            {pagedInstalled.map((m) => (
              <div
                key={m.fileName}
                className="flex items-center gap-2 rounded-lg border bg-card/60 p-2.5"
              >
                <input
                  type="checkbox"
                  checked={m.enabled}
                  onChange={() => toggle(m)}
                  className="h-4 w-4 accent-[hsl(var(--accent))]"
                  title={m.enabled ? "Disable" : "Enable"}
                />
                <InstalledIcon
                  iconUrl={m.projectId ? iconByProjectId.get(m.projectId) : null}
                  fallback={<Package className="h-4 w-4 text-muted-foreground" />}
                />
                <span
                  className={cn(
                    "flex-1 truncate text-sm",
                    !m.enabled && "text-muted-foreground line-through",
                  )}
                  title={m.fileName}
                >
                  {m.fileName.replace(/\.jar$/, "")}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {formatBytes(m.size)}
                </span>
                <button
                  onClick={() => remove(m)}
                  className="text-muted-foreground transition hover:text-destructive"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>
        )}
        <Pagination
          page={installedPage}
          pageCount={installedPageCount}
          onPage={setInstalledPage}
        />
      </section>

      {/* Browse */}
      <section>
        <div className="mb-3 flex items-center gap-2">
          <Input
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setPage(0);
            }}
            placeholder={`Search for ${instance.mcVersion} ${loaderLabel(instance.loader)} mods…`}
          />
          <ProviderToggle
            provider={provider}
            onChange={(p) => {
              setProvider(p);
              setPage(0);
            }}
          />
        </div>
        <ModrinthResultList
          results={results}
          searching={searching}
          installing={installing}
          onInstall={(hit) => setVersionPickTarget(hit)}
          provider={provider}
          emptyLabel="mods"
          fallbackIcon={<Package className="h-5 w-5 text-muted-foreground" />}
          installedIds={
            new Set(installed.map((m) => m.projectId).filter((x): x is string => !!x))
          }
        />
        <Pagination page={page} pageCount={pageCountFor(totalHits)} onPage={setPage} />
      </section>

      <VersionPickerModal
        hit={versionPickTarget}
        provider={provider}
        mcVersion={instance.mcVersion}
        loader={instance.loader}
        onClose={() => setVersionPickTarget(null)}
        onInstall={installWithVersion}
      />
    </div>
  );
}

// --------------------------------------------------------------------------
// Generic content browser panel (resource packs, shaders)
// --------------------------------------------------------------------------

interface ContentBrowserPanelProps {
  instance: Instance;
  contentType: string;
  modrinthType: string;
  useLoaderFilter: boolean;
  listItems: () => Promise<{ fileName: string; size: number; projectId?: string | null }[]>;
  deleteItem: (fileName: string) => Promise<void>;
  emptyLabel: string;
  placeholder: string;
}

function ContentBrowserPanel({
  instance,
  contentType,
  modrinthType,
  useLoaderFilter,
  listItems,
  deleteItem,
  emptyLabel,
  placeholder,
}: ContentBrowserPanelProps) {
  const toast = useStore((s) => s.toast);
  const [installed, setInstalled] = useState<
    { fileName: string; size: number; projectId?: string | null }[]
  >([]);
  const [installedQuery, setInstalledQuery] = useState("");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [page, setPage] = useState(0);
  const [totalHits, setTotalHits] = useState(0);

  const refresh = () => listItems().then(setInstalled).catch(() => {});
  useEffect(() => {
    setInstalledQuery("");
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id, contentType]);

  useEffect(() => {
    setSearching(true);
    const t = setTimeout(() => {
      searchProvider(provider, {
        query,
        contentType: modrinthType,
        loader: useLoaderFilter ? instance.loader : undefined,
        gameVersion: instance.mcVersion,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      })
        .then((r) => {
          setResults(r.hits);
          setTotalHits(r.total_hits);
        })
        .catch((e) => toast("error", errMessage(e)))
        .finally(() => setSearching(false));
    }, 350);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, provider, page, modrinthType, instance.mcVersion]);

  // Map projectId → icon from the current results for installed items.
  const iconByProjectId = new Map(
    results.filter((r) => r.icon_url).map((r) => [r.project_id, r.icon_url!]),
  );

  const filteredInstalled = useMemo(
    () => installed.filter((item) => matchesInstalledQuery(installedQuery, item.fileName, item.projectId)),
    [installed, installedQuery],
  );

  const {
    page: installedPage,
    setPage: setInstalledPage,
    pageCount: installedPageCount,
    pagedItems: pagedInstalled,
  } = useLocalPagination(filteredInstalled, `${instance.id}:${contentType}:${installedQuery}`);

  const [versionPickTarget, setVersionPickTarget] = useState<ModHit | null>(null);

  const installWithVersion = async (hit: ModHit, versionId: string) => {
    setInstalling(hit.project_id);
    setVersionPickTarget(null);
    try {
      const res =
        provider === "curseforge"
          ? await api.installCurseforgeFile({
              instanceId: instance.id,
              projectId: hit.project_id,
              fileId: versionId,
              contentType,
              loader: useLoaderFilter ? instance.loader : undefined,
              gameVersion: instance.mcVersion,
            })
          : await api.installContentVersion({
              instanceId: instance.id,
              projectId: hit.project_id,
              versionId,
              contentType,
              loader: useLoaderFilter ? instance.loader : undefined,
              gameVersion: instance.mcVersion,
            });
      const n = res.dependencies.length;
      toast(
        "success",
        n
          ? `Installed ${res.file} · +${n} ${n === 1 ? "dependency" : "dependencies"}`
          : `Installed ${res.file}`,
      );
      refresh();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setInstalling(null);
    }
  };

  const remove = async (fileName: string) => {
    await deleteItem(fileName).catch((e) => toast("error", errMessage(e)));
    refresh();
  };

  const { updates, checking, updatingAll, checkUpdates, applyAllUpdates } = useContentUpdates(
    instance,
    modrinthType,
    refresh,
  );

  const FallbackIcon =
    contentType === "shader" ? (
      <Sparkles className="h-5 w-5 text-muted-foreground" />
    ) : (
      <Image className="h-5 w-5 text-muted-foreground" />
    );

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      {/* Installed */}
      <section className="min-w-0">
        <div className="mb-3 flex items-center justify-between gap-2">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Installed ({filteredInstalled.length}
            {installedQuery.trim() ? ` / ${installed.length}` : ""})
          </h2>
          {installed.length > 0 && (
            <ContentUpdateButton checking={checking} onClick={checkUpdates} />
          )}
        </div>
        {installed.length > 0 && (
          <div className="relative mb-3">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={installedQuery}
              onChange={(e) => setInstalledQuery(e.target.value)}
              placeholder={`Search installed ${emptyLabel}…`}
              className="pl-9"
            />
          </div>
        )}
        <ContentUpdatesBanner
          updates={updates}
          updatingAll={updatingAll}
          onApply={applyAllUpdates}
        />
        {installed.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No {emptyLabel} yet. Search on the right to add some.
          </p>
        ) : filteredInstalled.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No installed {emptyLabel} match &ldquo;{installedQuery.trim()}&rdquo;.
          </p>
        ) : (
          <div className="space-y-1.5">
            {pagedInstalled.map((item) => (
              <div
                key={item.fileName}
                className="flex items-center gap-2 rounded-lg border bg-card/60 p-2.5"
              >
                <InstalledIcon
                  iconUrl={item.projectId ? iconByProjectId.get(item.projectId) : null}
                  fallback={FallbackIcon}
                />
                <span className="flex-1 truncate text-sm" title={item.fileName}>
                  {item.fileName.replace(/\.zip$/, "")}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {formatBytes(item.size)}
                </span>
                <button
                  onClick={() => remove(item.fileName)}
                  className="text-muted-foreground transition hover:text-destructive"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>
        )}
        <Pagination
          page={installedPage}
          pageCount={installedPageCount}
          onPage={setInstalledPage}
        />
      </section>

      {/* Browse */}
      <section>
        <div className="mb-3 flex items-center gap-2">
          <Input
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setPage(0);
            }}
            placeholder={placeholder}
          />
          <ProviderToggle
            provider={provider}
            onChange={(p) => {
              setProvider(p);
              setPage(0);
            }}
          />
        </div>
        <ModrinthResultList
          results={results}
          searching={searching}
          installing={installing}
          onInstall={(hit) => setVersionPickTarget(hit)}
          provider={provider}
          emptyLabel={emptyLabel}
          fallbackIcon={FallbackIcon}
          installedIds={
            new Set(installed.map((m) => m.projectId).filter((x): x is string => !!x))
          }
        />
        <Pagination page={page} pageCount={pageCountFor(totalHits)} onPage={setPage} />
      </section>

      <VersionPickerModal
        hit={versionPickTarget}
        provider={provider}
        mcVersion={instance.mcVersion}
        loader={useLoaderFilter ? instance.loader : "vanilla"}
        onClose={() => setVersionPickTarget(null)}
        onInstall={installWithVersion}
      />
    </div>
  );
}

// --------------------------------------------------------------------------
// Shared Modrinth result list
// --------------------------------------------------------------------------

interface ModrinthResultListProps {
  results: ModHit[];
  searching: boolean;
  installing: string | null;
  onInstall: (hit: ModHit) => void;
  emptyLabel: string;
  fallbackIcon: ReactNode;
  installedIds?: Set<string>;
  provider?: Provider;
}

function ModrinthResultList({
  results,
  searching,
  installing,
  onInstall,
  emptyLabel,
  fallbackIcon,
  installedIds,
  provider,
}: ModrinthResultListProps) {
  if (searching && results.length === 0) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 5 }).map((_, i) => (
          <div key={i} className="flex items-start gap-3 rounded-xl border bg-card/60 p-3">
            <Skeleton className="h-11 w-11 rounded-lg" />
            <div className="flex-1 space-y-2 py-1">
              <Skeleton className="h-3.5 w-1/3" />
              <Skeleton className="h-3 w-3/4" />
            </div>
            <Skeleton className="h-8 w-16 rounded-md" />
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="space-y-2">
      {results.map((hit) => {
        const isInstalling = installing === hit.project_id;
        const isInstalled = installedIds?.has(hit.project_id) ?? false;
        return (
          <div
            key={hit.project_id}
            className="flex items-start gap-3 rounded-xl border bg-card/60 p-3 transition hover:bg-card"
          >
            {hit.icon_url ? (
              <img
                src={hit.icon_url}
                alt=""
                className="h-11 w-11 shrink-0 rounded-lg object-cover"
              />
            ) : (
              <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-muted">
                {fallbackIcon}
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline gap-2">
                <span className="truncate font-medium">{hit.title}</span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {formatNumber(hit.downloads)} ↓
                </span>
              </div>
              <p className="mt-0.5 line-clamp-2 text-sm text-muted-foreground">{hit.description}</p>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              {provider && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    api.openUrl(contentUrl(hit, provider));
                  }}
                  className="text-muted-foreground transition hover:text-foreground"
                  title={`Open on ${provider === "curseforge" ? "CurseForge" : "Modrinth"}`}
                >
                  <ExternalLink className="h-3.5 w-3.5" />
                </button>
              )}
              {isInstalled ? (
                <span className="inline-flex items-center gap-1 rounded-md bg-emerald-500/15 px-2.5 py-1 text-xs font-medium text-emerald-400">
                  <Check className="h-3.5 w-3.5" />
                  Installed
                </span>
              ) : (
                <Button
                  size="sm"
                  variant="secondary"
                  loading={isInstalling}
                  onClick={() => onInstall(hit)}
                >
                  {!isInstalling && <Download className="h-3.5 w-3.5" />}
                  Add
                </Button>
              )}
            </div>
          </div>
        );
      })}
      {!searching && results.length === 0 && (
        <p className="py-10 text-center text-sm text-muted-foreground">
          No {emptyLabel} found.
        </p>
      )}
    </div>
  );
}

// --------------------------------------------------------------------------
// Version picker modal — shown when "Add" is clicked on a mod result
// --------------------------------------------------------------------------

/** True when a version targets this instance's MC version and mod loader. */
function versionFitsInstance(v: ContentVersion, mcVersion: string, loader: string): boolean {
  const mcOk = v.gameVersions.includes(mcVersion);
  if (loader === "vanilla") return mcOk;
  const ld = v.loaders.map((x) => x.toLowerCase());
  const gv = v.gameVersions.map((x) => x.toLowerCase());
  // Modrinth lists loaders explicitly; CurseForge folds them into gameVersions.
  const loaderOk = ld.includes(loader) || gv.includes(loader);
  return mcOk && loaderOk;
}

function VersionPickerModal({
  hit,
  provider,
  mcVersion,
  loader,
  onClose,
  onInstall,
}: {
  hit: ModHit | null;
  provider: Provider;
  mcVersion: string;
  loader: string;
  onClose: () => void;
  onInstall: (hit: ModHit, versionId: string) => void;
}) {
  const toast = useStore((s) => s.toast);
  const [versions, setVersions] = useState<ContentVersion[]>([]);
  const [versionId, setVersionId] = useState("");
  const [loading, setLoading] = useState(false);

  // The newest version that fits this instance — the one we recommend + preselect.
  const recommendedId = useMemo(() => {
    const match = versions.find((v) => versionFitsInstance(v, mcVersion, loader));
    return match?.id ?? "";
  }, [versions, mcVersion, loader]);

  // Sort compatible versions to the top, keeping newest-first order within groups.
  const ordered = useMemo(() => {
    const fits = (v: ContentVersion) => versionFitsInstance(v, mcVersion, loader);
    return [...versions].sort((a, b) => Number(fits(b)) - Number(fits(a)));
  }, [versions, mcVersion, loader]);

  useEffect(() => {
    if (!hit) return;
    setVersions([]);
    setVersionId("");
    setLoading(true);
    const fetcher =
      provider === "curseforge"
        ? api.listCurseforgeFiles(hit.project_id)
        : api.listModrinthVersions(hit.project_id);
    fetcher
      .then((v) => setVersions(v))
      .catch((e) => toast("error", errMessage(e)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hit?.project_id, provider]);

  // Default to the recommended version once versions load; fall back to newest.
  useEffect(() => {
    if (versions.length === 0) return;
    setVersionId(recommendedId || versions[0].id);
  }, [versions, recommendedId]);

  return (
    <Modal
      open={!!hit}
      onClose={onClose}
      title="Choose version"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            disabled={loading || !versionId}
            onClick={() => hit && versionId && onInstall(hit, versionId)}
          >
            <Download className="h-4 w-4" />
            Install
          </Button>
        </>
      }
    >
      {hit && (
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            {hit.icon_url ? (
              <img src={hit.icon_url} alt="" className="h-12 w-12 rounded-lg object-cover" />
            ) : (
              <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-muted">
                <Package className="h-5 w-5 text-muted-foreground" />
              </div>
            )}
            <div className="min-w-0">
              <p className="truncate font-medium">{hit.title}</p>
              {hit.author && <p className="text-xs text-muted-foreground">{hit.author}</p>}
            </div>
          </div>

          <div>
            <span className="mb-1.5 block text-sm font-medium">Version</span>
            {loading ? (
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Spinner className="h-4 w-4" />
                Loading versions…
              </div>
            ) : versions.length === 0 ? (
              <p className="text-sm text-muted-foreground">No versions found.</p>
            ) : (
              <>
                <Select value={versionId} onChange={(e) => setVersionId(e.target.value)}>
                  {ordered.map((v) => {
                    const mc = v.gameVersions.includes(mcVersion)
                      ? mcVersion
                      : v.gameVersions[0] ?? "?";
                    const recommended = v.id === recommendedId;
                    const fits = versionFitsInstance(v, mcVersion, loader);
                    return (
                      <option key={v.id} value={v.id}>
                        {recommended ? "★ " : ""}
                        {v.name || v.versionNumber} — MC {mc}
                        {!fits ? " (other)" : ""} ({new Date(v.date).toLocaleDateString()})
                      </option>
                    );
                  })}
                </Select>
                <p className="mt-1.5 text-xs text-muted-foreground">
                  {recommendedId
                    ? `★ Recommended for MC ${mcVersion} · ${loaderLabel(loader as Instance["loader"])}`
                    : `No version explicitly lists MC ${mcVersion} — showing newest.`}
                </p>
              </>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

// --------------------------------------------------------------------------
// Worlds panel
// --------------------------------------------------------------------------

function WorldsPanel({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const running = useStore((s) => s.running.has(instance.id));
  const [worlds, setWorlds] = useState<WorldEntry[]>([]);
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const refresh = () => {
    api.listWorlds(instance.id).then(setWorlds).catch(() => {});
    api.listSnapshots(instance.id).then(setSnapshots).catch(() => {});
  };
  useEffect(() => {
    setQuery("");
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  const filteredWorlds = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return worlds;
    return worlds.filter((w) => w.name.toLowerCase().includes(q));
  }, [worlds, query]);

  const {
    page: worldsPage,
    setPage: setWorldsPage,
    pageCount: worldsPageCount,
    pagedItems: pagedWorlds,
  } = useLocalPagination(filteredWorlds, `${instance.id}:${query}`);

  const remove = async (name: string) => {
    if (!confirm(`Delete world "${name}"? This cannot be undone.`)) return;
    await api.deleteWorld(instance.id, name).catch((e) => toast("error", errMessage(e)));
    refresh();
  };

  // Quick Play — launch straight into this world, skipping the main menu.
  const playWorld = async (name: string) => {
    if (running) {
      toast("error", "This instance is already running.");
      return;
    }
    try {
      await api.launchInstance(instance.id, { world: name });
      toast("info", `Launching into “${name}”…`);
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const backup = async (name: string) => {
    setBusy(name);
    try {
      await api.createSnapshot(instance.id, name);
      toast("success", `Backed up “${name}”`);
      refresh();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const restore = async (s: Snapshot) => {
    if (!confirm(`Restore “${s.world}” from ${new Date(s.created * 1000).toLocaleString()}? This overwrites the current world.`))
      return;
    setBusy(s.fileName);
    try {
      await api.restoreSnapshot(instance.id, s.fileName);
      toast("success", `Restored “${s.world}”`);
      refresh();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const deleteSnapshot = async (s: Snapshot) => {
    await api.deleteSnapshot(instance.id, s.fileName).catch((e) => toast("error", errMessage(e)));
    refresh();
  };

  return (
    <div className="max-w-xl space-y-6">
      {worlds.length > 0 && (
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search worlds…"
            className="pl-9"
          />
        </div>
      )}
      <div className="space-y-1.5">
        {worlds.length === 0 ? (
          <EmptyState
            icon={<Globe className="h-7 w-7" />}
            title="No worlds"
            description="Worlds will appear here once you play and create or load one."
          />
        ) : filteredWorlds.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No worlds match &ldquo;{query.trim()}&rdquo;.
          </p>
        ) : (
          pagedWorlds.map((w) => (
            <div
              key={w.name}
              className="flex items-center gap-3 rounded-lg border bg-card/60 p-3"
            >
              <Globe className="h-5 w-5 shrink-0 text-muted-foreground" />
              <div className="min-w-0 flex-1">
                <p className="truncate font-medium text-sm">{w.name}</p>
                {w.modified && (
                  <p className="text-xs text-muted-foreground">
                    {new Date(w.modified * 1000).toLocaleDateString()}
                  </p>
                )}
              </div>
              <button
                onClick={() => playWorld(w.name)}
                disabled={running}
                className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-accent transition hover:bg-accent/10 disabled:opacity-40"
                title="Play this world"
              >
                <Play className="h-3.5 w-3.5 fill-current" />
                Play
              </button>
              <button
                onClick={() => backup(w.name)}
                disabled={busy === w.name}
                className="text-muted-foreground transition hover:text-foreground disabled:opacity-40"
                title="Back up this world"
              >
                <Save className="h-4 w-4" />
              </button>
              <button
                onClick={() => api.openWorldFolder(instance.id, w.name)}
                className="text-muted-foreground transition hover:text-foreground"
                title="Open folder"
              >
                <ExternalLink className="h-4 w-4" />
              </button>
              <button
                onClick={() => remove(w.name)}
                className="text-muted-foreground transition hover:text-destructive"
                title="Delete world"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          ))
        )}
      </div>
      <Pagination page={worldsPage} pageCount={worldsPageCount} onPage={setWorldsPage} />

      {snapshots.length > 0 && (
        <section>
          <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            <Save className="h-3.5 w-3.5" />
            Backups
            <span className="opacity-60">{snapshots.length}</span>
          </h3>
          <div className="space-y-1.5">
            {snapshots.map((s) => (
              <div
                key={s.fileName}
                className="flex items-center gap-3 rounded-lg border bg-card/40 p-2.5"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{s.world}</p>
                  <p className="text-xs text-muted-foreground">
                    {new Date(s.created * 1000).toLocaleString()} · {formatBytes(s.size)}
                  </p>
                </div>
                <button
                  onClick={() => restore(s)}
                  disabled={busy === s.fileName}
                  className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-muted-foreground transition hover:text-foreground disabled:opacity-40"
                  title="Restore this backup"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                  Restore
                </button>
                <button
                  onClick={() => deleteSnapshot(s)}
                  className="text-muted-foreground transition hover:text-destructive"
                  title="Delete backup"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

// --------------------------------------------------------------------------
// Screenshots panel
// --------------------------------------------------------------------------

function ScreenshotsPanel({ instance }: { instance: Instance }) {
  const [screenshots, setScreenshots] = useState<ScreenshotEntry[]>([]);
  const [query, setQuery] = useState("");

  useEffect(() => {
    setQuery("");
    api.listScreenshots(instance.id).then(setScreenshots).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  const filteredScreenshots = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return screenshots;
    return screenshots.filter((s) => s.fileName.toLowerCase().includes(q));
  }, [screenshots, query]);

  const {
    page: shotsPage,
    setPage: setShotsPage,
    pageCount: shotsPageCount,
    pagedItems: pagedScreenshots,
  } = useLocalPagination(filteredScreenshots, `${instance.id}:${query}`);

  if (screenshots.length === 0) {
    return (
      <EmptyState
        icon={<Camera className="h-7 w-7" />}
        title="No screenshots"
        description="Screenshots taken in-game (F2) will appear here."
      />
    );
  }

  return (
    <div>
      <div className="relative mb-4 max-w-md">
        <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search screenshots…"
          className="pl-9"
        />
      </div>
      {filteredScreenshots.length === 0 ? (
        <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
          No screenshots match &ldquo;{query.trim()}&rdquo;.
        </p>
      ) : (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
          {pagedScreenshots.map((s) => (
            <button
              key={s.fileName}
              onClick={() => api.openScreenshot(instance.id, s.fileName)}
              className="group relative flex flex-col rounded-lg border bg-card/60 p-2.5 text-left transition hover:bg-card btn-focus"
              title="Open in viewer"
            >
              <div className="mb-2 flex h-24 items-center justify-center rounded-md bg-muted">
                <Camera className="h-6 w-6 text-muted-foreground" />
              </div>
              <p className="truncate text-xs font-medium" title={s.fileName}>
                {s.fileName}
              </p>
              <p className="text-xs text-muted-foreground">
                {new Date(s.takenAt * 1000).toLocaleDateString()}
              </p>
              <ExternalLink className="absolute right-2 top-2 h-3.5 w-3.5 text-muted-foreground opacity-0 transition group-hover:opacity-100" />
            </button>
          ))}
        </div>
      )}
      <Pagination page={shotsPage} pageCount={shotsPageCount} onPage={setShotsPage} />
    </div>
  );
}

// --------------------------------------------------------------------------
// Servers — saved multiplayer list with live status (MOTD, players, latency)
// --------------------------------------------------------------------------

/** Tailwind bg-* class for a latency dot: green < 100ms, amber < 300ms, red. */
function latencyColor(ms?: number | null): string {
  if (ms == null) return "bg-muted-foreground";
  if (ms < 100) return "bg-emerald-400";
  if (ms < 300) return "bg-amber-400";
  return "bg-red-400";
}

function ServersTab({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const running = useStore((s) => s.running.has(instance.id));
  const [servers, setServers] = useState<SavedServer[]>([]);
  const [statuses, setStatuses] = useState<Record<string, ServerStatus | null>>({});
  const [pinging, setPinging] = useState<Record<string, boolean>>({});
  const [query, setQuery] = useState("");
  const [manual, setManual] = useState<{ address: string; status: ServerStatus } | null>(null);
  const [manualBusy, setManualBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);

  const ping = async (address: string) => {
    setPinging((m) => ({ ...m, [address]: true }));
    try {
      const st = await api.pingServer(address);
      setStatuses((m) => ({ ...m, [address]: st }));
    } catch {
      setStatuses((m) => ({ ...m, [address]: { online: false } }));
    } finally {
      setPinging((m) => ({ ...m, [address]: false }));
    }
  };

  useEffect(() => {
    setLoaded(false);
    setStatuses({});
    api
      .listServers(instance.id)
      .then((list) => {
        setServers(list);
        list.forEach((s) => ping(s.ip));
      })
      .catch((e) => toast("error", errMessage(e)))
      .finally(() => setLoaded(true));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  const pingManual = async () => {
    const address = query.trim();
    if (!address) return;
    setManualBusy(true);
    try {
      setManual({ address, status: await api.pingServer(address) });
    } finally {
      setManualBusy(false);
    }
  };

  const connect = async (ip: string) => {
    if (running) {
      toast("error", "This instance is already running.");
      return;
    }
    try {
      await api.launchInstance(instance.id, { server: ip });
      toast("info", `Connecting to ${ip}…`);
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const copy = async (ip: string) => {
    try {
      await navigator.clipboard.writeText(ip);
      toast("success", "Address copied");
    } catch {
      /* clipboard unavailable */
    }
  };

  return (
    <div className="max-w-2xl space-y-5">
      {/* Ping / connect to any address */}
      <div className="rounded-xl border bg-card/40 p-4">
        <h3 className="mb-2 flex items-center gap-1.5 text-sm font-semibold">
          <Wifi className="h-4 w-4 text-accent" />
          Ping en server
        </h3>
        <p className="mb-2 text-xs text-muted-foreground">
          Teksten under serveren i spillets multiplayer-meny kommer fra serverens MOTD (satt i{" "}
          <code className="rounded bg-muted px-1 py-0.5">server.properties</code>).
        </p>
        <div className="flex gap-2">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && pingManual()}
            placeholder="135.125.201.56   eller   mc.host.com:25566"
          />
          <Button variant="secondary" onClick={pingManual} loading={manualBusy}>
            Ping
          </Button>
        </div>
        {manual && (
          <div className="mt-3">
            <ServerRow
              server={{ name: manual.address, ip: manual.address, icon: null }}
              status={manual.status}
              pinging={false}
              running={running}
              onRefresh={pingManual}
              onConnect={() => connect(manual.address)}
              onCopy={() => copy(manual.address)}
            />
          </div>
        )}
      </div>

      {/* Saved servers (from servers.dat) */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Lagrede servere ({servers.length})
          </h3>
          {servers.length > 0 && (
            <button
              onClick={() => servers.forEach((s) => ping(s.ip))}
              className="inline-flex items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus"
            >
              <RefreshCw className="h-3 w-3" /> Oppdater alle
            </button>
          )}
        </div>

        {!loaded ? (
          <div className="space-y-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="flex items-start gap-3 rounded-xl border bg-card/60 p-3">
                <Skeleton className="h-11 w-11 rounded-lg" />
                <div className="flex-1 space-y-2 py-1">
                  <Skeleton className="h-3.5 w-1/4" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              </div>
            ))}
          </div>
        ) : servers.length === 0 ? (
          <EmptyState
            icon={<Server className="h-7 w-7" />}
            title="Ingen lagrede servere"
            description="Servere du legger til i spillets multiplayer-liste vises her med live status. Du kan også ping en adresse over."
          />
        ) : (
          <div className="space-y-2">
            {servers.map((s) => (
              <ServerRow
                key={`${s.ip}__${s.name}`}
                server={s}
                status={statuses[s.ip] ?? null}
                pinging={!!pinging[s.ip]}
                running={running}
                onRefresh={() => ping(s.ip)}
                onConnect={() => connect(s.ip)}
                onCopy={() => copy(s.ip)}
              />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function ServerRow({
  server,
  status,
  pinging,
  running,
  onRefresh,
  onConnect,
  onCopy,
}: {
  server: SavedServer;
  status: ServerStatus | null;
  pinging: boolean;
  running: boolean;
  onRefresh: () => void;
  onConnect: () => void;
  onCopy: () => void;
}) {
  const favicon =
    status?.favicon ?? (server.icon ? `data:image/png;base64,${server.icon}` : null);
  const online = status?.online ?? false;

  return (
    <div className="flex items-start gap-3 rounded-xl border bg-card/60 p-3">
      {favicon ? (
        <img src={favicon} alt="" className="h-11 w-11 shrink-0 rounded-lg object-cover" />
      ) : (
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Server className="h-5 w-5 text-muted-foreground" />
        </div>
      )}

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium">{server.name || server.ip}</span>
          {pinging ? (
            <Spinner className="h-3 w-3" />
          ) : (
            <span
              className={cn(
                "h-2 w-2 shrink-0 rounded-full",
                online ? latencyColor(status?.latencyMs) : "bg-muted-foreground/40",
              )}
              title={online ? "Online" : "Offline"}
            />
          )}
        </div>
        {status?.motd ? (
          <p className="line-clamp-2 text-xs text-muted-foreground">{status.motd}</p>
        ) : pinging ? (
          <p className="text-xs text-muted-foreground/70">Pinger…</p>
        ) : !online ? (
          <p className="text-xs text-muted-foreground/70">Offline eller utilgjengelig</p>
        ) : null}
        <p className="truncate text-xs text-muted-foreground/70">{server.ip}</p>
      </div>

      <div className="flex shrink-0 flex-col items-end gap-1.5">
        {online && (
          <div className="flex items-center gap-2.5 text-xs">
            {status?.playersOnline != null && (
              <span className="inline-flex items-center gap-1 text-muted-foreground">
                <Users className="h-3 w-3" />
                {status.playersOnline}
                {status?.playersMax != null ? `/${status.playersMax}` : ""}
              </span>
            )}
            {status?.latencyMs != null && (
              <span className="text-muted-foreground">{status.latencyMs} ms</span>
            )}
          </div>
        )}
        <div className="flex items-center gap-1">
          <button
            onClick={onCopy}
            title="Copy address"
            className="rounded-md p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground btn-focus"
          >
            <Copy className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={onRefresh}
            title="Re-ping"
            disabled={pinging}
            className="rounded-md p-1.5 text-muted-foreground transition hover:bg-muted hover:text-foreground btn-focus disabled:opacity-40"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", pinging && "animate-spin")} />
          </button>
          <Button
            size="sm"
            variant="secondary"
            onClick={onConnect}
            disabled={running}
            title="Start og koble til"
          >
            <Play className="h-3.5 w-3.5 fill-current" />
            Koble til
          </Button>
        </div>
      </div>
    </div>
  );
}

// --------------------------------------------------------------------------
// Logs
// --------------------------------------------------------------------------

function LogsTab({
  id,
  instance,
  onOpenSettings,
}: {
  id: string;
  instance: Instance;
  onOpenSettings: () => void;
}) {
  const logs = useStore((s) => s.logs[id] ?? []);
  const clearLogs = useStore((s) => s.clearLogs);
  const updateInstance = useStore((s) => s.updateInstance);
  const toast = useStore((s) => s.toast);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [sharing, setSharing] = useState(false);

  const logText = () => logs.map((l) => l.line).join("\n");
  const copyLog = async () => {
    try {
      await navigator.clipboard.writeText(logText());
      toast("success", "Log copied");
    } catch {
      /* clipboard unavailable */
    }
  };
  const shareLog = async () => {
    if (logs.length === 0) return;
    setSharing(true);
    try {
      const url = await api.uploadLog(logText());
      await navigator.clipboard.writeText(url).catch(() => {});
      toast("success", "Log uploaded — link copied");
      api.openUrl(url);
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setSharing(false);
    }
  };

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs.length]);

  // Re-run only when new lines arrive; cheap enough for the capped log buffer.
  const finding = useMemo(() => analyzeCrash(logs), [logs]);

  const applyAction = async (action: NonNullable<CrashFinding["action"]>) => {
    if (action === "increase-ram") {
      const next = (instance.memoryMb ?? 4096) + 2048;
      await updateInstance({ ...instance, memoryMb: next });
      toast("success", `Raised memory to ${(next / 1024).toFixed(0)} GB`);
    } else if (action === "open-settings") {
      onOpenSettings();
    }
  };

  return (
    <div className="flex min-h-0 flex-col">
      {finding && (
        <div className="mb-3 rounded-xl border border-amber-500/40 bg-amber-500/10 p-3">
          <div className="flex items-start gap-2.5">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-400" />
            <div className="flex-1">
              <p className="text-sm font-semibold text-amber-200">{finding.title}</p>
              <p className="mt-0.5 text-xs text-muted-foreground">{finding.detail}</p>
              {finding.action && (
                <Button
                  size="sm"
                  variant="secondary"
                  className="mt-2"
                  onClick={() => applyAction(finding.action!)}
                >
                  {finding.action === "increase-ram" ? "Add 2 GB RAM" : "Open settings"}
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
      <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
        <span className="text-sm text-muted-foreground">{logs.length} lines</span>
        <div className="flex flex-wrap items-center gap-1">
          <Button size="sm" variant="ghost" onClick={copyLog} disabled={logs.length === 0}>
            <Copy className="h-3.5 w-3.5" /> Copy
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={shareLog}
            loading={sharing}
            disabled={logs.length === 0}
            title="Upload to mclo.gs and copy the link"
          >
            <Upload className="h-3.5 w-3.5" /> Share
          </Button>
          <Button size="sm" variant="ghost" onClick={() => clearLogs(id)}>
            Clear
          </Button>
        </div>
      </div>
      <div className="app-scroll min-h-[240px] max-h-[min(70vh,640px)] rounded-xl border bg-[hsl(240_12%_4%)] p-3 font-mono text-xs leading-relaxed">
        {logs.length === 0 ? (
          <p className="text-muted-foreground">
            No output yet. Press Play to launch and the game log will stream here.
          </p>
        ) : (
          logs.map((l, i) => (
            <div
              key={i}
              className={cn("whitespace-pre-wrap break-all", l.isErr ? "text-red-400" : "text-zinc-300")}
            >
              {l.line}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}

// --------------------------------------------------------------------------
// Settings
// --------------------------------------------------------------------------

function SettingsTab({ instance }: { instance: Instance }) {
  const update = useStore((s) => s.updateInstance);
  const remove = useStore((s) => s.deleteInstance);
  const running = useStore((s) => s.running.has(instance.id));
  const toast = useStore((s) => s.toast);

  const [name, setName] = useState(instance.name);
  const [group, setGroup] = useState(instance.group ?? "");
  const [accent, setAccent] = useState(instance.accent ?? "");
  // Modpack art (an image icon) is left untouched; emoji / Auto stay editable.
  const hasImageIcon = isImageIcon(instance.icon);
  const [icon, setIcon] = useState(hasImageIcon ? "" : instance.icon ?? "");
  const [memory, setMemory] = useState(instance.memoryMb ?? 0);
  const [javaPath, setJavaPath] = useState(instance.javaPath ?? "");
  const [jvmArgs, setJvmArgs] = useState(instance.jvmArgs ?? "");
  const [winW, setWinW] = useState(instance.windowWidth ?? 0);
  const [winH, setWinH] = useState(instance.windowHeight ?? 0);
  const [envVars, setEnvVars] = useState(instance.envVars ?? "");
  const [preLaunch, setPreLaunch] = useState(instance.preLaunch ?? "");
  const [postExit, setPostExit] = useState(instance.postExit ?? "");
  const [saving, setSaving] = useState(false);
  const [exportingPack, setExportingPack] = useState(false);
  const [usage, setUsage] = useState<DiskUsage | null>(null);

  useEffect(() => {
    api.instanceDiskUsage(instance.id).then(setUsage).catch(() => {});
  }, [instance.id]);

  const exportPack = async () => {
    try {
      setExportingPack(true);
      toast("info", "Exporting modpack…");
      const path = await exportInstanceMrpack(instance.id, instance.name);
      if (path) toast("success", "Exported .mrpack");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setExportingPack(false);
    }
  };

  const saveChanges = async () => {
    setSaving(true);
    try {
      await update({
        ...instance,
        name: name.trim() || instance.name,
        group: group.trim() || null,
        accent: accent || null,
        icon: hasImageIcon ? instance.icon : icon || null,
        memoryMb: memory > 0 ? memory : null,
        javaPath: javaPath.trim() || null,
        jvmArgs: jvmArgs.trim() || null,
        windowWidth: winW > 0 ? winW : null,
        windowHeight: winH > 0 ? winH : null,
        envVars: envVars.trim() || null,
        preLaunch: preLaunch.trim() || null,
        postExit: postExit.trim() || null,
      });
      toast("success", "Saved");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="max-w-xl space-y-5">
      <Field label="Name">
        <Input value={name} onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Group" hint="Optional. Instances with the same group are shown together.">
        <Input
          value={group}
          onChange={(e) => setGroup(e.target.value)}
          placeholder="e.g. Modded, Vanilla, Servers…"
        />
      </Field>
      {!hasImageIcon && (
        <Field label="Icon" hint="Auto uses the mod loader's logo.">
          <div className="flex flex-wrap items-center gap-1.5">
            <button
              type="button"
              onClick={() => setIcon("")}
              title="Auto — use the loader logo"
              className={cn(
                "flex h-9 w-9 items-center justify-center rounded-lg transition",
                icon === "" ? "bg-accent/20 ring-2 ring-accent" : "bg-muted/60 hover:bg-muted",
              )}
            >
              <LoaderLogo loader={instance.loader} className="h-6 w-6" />
            </button>
            {["🟩", "🔥", "⚙️", "🧪", "🏰", "🌲", "💎", "🚀", "🐉", "⛏️", "🧱", "✨"].map((e) => (
              <button
                key={e}
                type="button"
                onClick={() => setIcon(e)}
                className={cn(
                  "flex h-9 w-9 items-center justify-center rounded-lg text-lg transition",
                  icon === e ? "bg-accent/20 ring-2 ring-accent" : "bg-muted/60 hover:bg-muted",
                )}
              >
                {e}
              </button>
            ))}
          </div>
        </Field>
      )}
      <Field label="Card colour" hint="Tint this instance's card. Auto follows the mod loader.">
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => setAccent("")}
            className={cn(
              "h-7 rounded-md border px-2 text-xs transition",
              accent === "" ? "border-accent text-foreground" : "border-border text-muted-foreground",
            )}
          >
            Auto
          </button>
          {Object.entries(ACCENTS).map(([key, hsl]) => (
            <button
              key={key}
              type="button"
              onClick={() => setAccent(key)}
              title={key}
              style={{ backgroundColor: `hsl(${hsl})` }}
              className={cn(
                "h-7 w-7 rounded-md ring-2 ring-offset-2 ring-offset-background transition",
                accent === key ? "ring-foreground/60" : "ring-transparent",
              )}
            />
          ))}
        </div>
      </Field>
      <Field
        label="Memory override (MB)"
        hint="Leave 0 to use the global setting."
      >
        <Input
          type="number"
          value={memory}
          onChange={(e) => setMemory(Number(e.target.value))}
          min={0}
          step={512}
        />
      </Field>
      <Field label="Java path override" hint="Leave empty to auto-detect / use the global setting.">
        <Input
          value={javaPath}
          onChange={(e) => setJavaPath(e.target.value)}
          placeholder="C:\\path\\to\\java.exe"
        />
      </Field>
      <Field label="Extra JVM arguments override">
        <div className="mb-2 flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onClick={() => setJvmArgs(AIKAR_FLAGS)}
            className={cn(
              "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition btn-focus",
              jvmArgs.trim() === AIKAR_FLAGS
                ? "bg-accent/20 text-accent"
                : "bg-muted/60 text-muted-foreground hover:text-foreground",
            )}
          >
            <Sparkles className="h-3 w-3" />
            {jvmArgs.trim() === AIKAR_FLAGS ? "Aikar's flags applied" : "Use Aikar's flags"}
          </button>
          {jvmArgs.trim() !== "" && (
            <button
              type="button"
              onClick={() => setJvmArgs("")}
              className="rounded-md px-2 py-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus"
            >
              Clear
            </button>
          )}
        </div>
        <textarea
          value={jvmArgs}
          onChange={(e) => setJvmArgs(e.target.value)}
          rows={3}
          placeholder="Leave empty to use the global JVM arguments."
          className="input-base resize-none font-mono text-xs"
        />
      </Field>

      <Field label="Game window size" hint="Leave 0 to use Minecraft's default / last size.">
        <div className="flex items-center gap-2">
          <Input
            type="number"
            value={winW}
            onChange={(e) => setWinW(Number(e.target.value))}
            min={0}
            step={1}
            placeholder="Width"
          />
          <span className="text-muted-foreground">×</span>
          <Input
            type="number"
            value={winH}
            onChange={(e) => setWinH(Number(e.target.value))}
            min={0}
            step={1}
            placeholder="Height"
          />
        </div>
      </Field>

      <Field label="Environment variables" hint="One KEY=VALUE per line, set on the game process.">
        <textarea
          value={envVars}
          onChange={(e) => setEnvVars(e.target.value)}
          rows={2}
          placeholder={"MESA_GL_VERSION_OVERRIDE=4.6"}
          className="input-base resize-none font-mono text-xs"
        />
      </Field>

      <Field label="Pre-launch command" hint="Runs before the game starts (blocking).">
        <Input
          value={preLaunch}
          onChange={(e) => setPreLaunch(e.target.value)}
          placeholder="Optional shell command"
          className="font-mono text-xs"
        />
      </Field>

      <Field label="Post-exit command" hint="Runs after the game closes.">
        <Input
          value={postExit}
          onChange={(e) => setPostExit(e.target.value)}
          placeholder="Optional shell command"
          className="font-mono text-xs"
        />
      </Field>

      <div className="flex justify-end">
        <Button variant="primary" onClick={saveChanges} loading={saving}>
          Save changes
        </Button>
      </div>

      {usage && usage.total > 0 && (
        <div className="rounded-xl border bg-card/40 p-4">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="text-sm font-semibold">Disk usage</h3>
            <span className="text-sm text-muted-foreground">{formatBytes(usage.total)}</span>
          </div>
          <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-muted">
            {(
              [
                ["mods", usage.mods, "bg-blue-500"],
                ["saves", usage.saves, "bg-emerald-500"],
                ["resourcepacks", usage.resourcepacks, "bg-violet-500"],
                ["shaders", usage.shaders, "bg-amber-500"],
                ["other", usage.other, "bg-zinc-500"],
              ] as const
            ).map(([key, val, color]) => (
              <div
                key={key}
                className={color}
                style={{ width: `${(val / usage.total) * 100}%` }}
                title={`${key}: ${formatBytes(val)}`}
              />
            ))}
          </div>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            {(
              [
                ["Mods", usage.mods, "bg-blue-500"],
                ["Worlds", usage.saves, "bg-emerald-500"],
                ["Resource packs", usage.resourcepacks, "bg-violet-500"],
                ["Shaders", usage.shaders, "bg-amber-500"],
                ["Other", usage.other, "bg-zinc-500"],
              ] as const
            )
              .filter(([, val]) => val > 0)
              .map(([label, val, color]) => (
                <span key={label} className="flex items-center gap-1.5">
                  <span className={cn("h-2 w-2 rounded-sm", color)} />
                  {label} {formatBytes(val)}
                </span>
              ))}
          </div>
        </div>
      )}

      <div className="rounded-xl border bg-card/40 p-4">
        <h3 className="text-sm font-semibold">Share</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Export this instance as a Modrinth modpack (.mrpack) others can install.
          Modrinth content becomes download links; CurseForge/local files, configs,
          and folder-based packs are bundled in.
        </p>
        <Button
          variant="secondary"
          className="mt-3"
          loading={exportingPack}
          onClick={exportPack}
        >
          <Upload className="h-4 w-4" /> Export as .mrpack
        </Button>
      </div>

      <div className="mt-8 rounded-xl border border-destructive/30 bg-destructive/5 p-4">
        <h3 className="text-sm font-semibold text-destructive">Danger zone</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Permanently delete this instance and all of its files.
        </p>
        <Button
          variant="danger"
          className="mt-3"
          onClick={() => {
            if (running) {
              toast("error", "Stop the instance before deleting it.");
              return;
            }
            if (confirm(`Delete “${instance.name}”? This cannot be undone.`)) remove(instance.id);
          }}
        >
          <Trash2 className="h-4 w-4" /> Delete instance
        </Button>
      </div>
    </div>
  );
}

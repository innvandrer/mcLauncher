import { type ReactNode, useEffect, useRef, useState } from "react";
import {
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
  ScrollText,
  Settings2,
  Sparkles,
  Square,
  Trash2,
  Upload,
} from "lucide-react";
import { Button, EmptyState, Field, Input, Modal, Select, Spinner } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import { cn, formatBytes, formatNumber, loaderLabel } from "@/lib/utils";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  ContentVersion,
  Instance,
  ModEntry,
  ModHit,
  ModUpdate,
  ResourcePackEntry,
  ScreenshotEntry,
  ShaderEntry,
  WorldEntry,
} from "@/lib/types";

type Tab = "content" | "logs" | "settings";

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
  const [exporting, setExporting] = useState(false);

  if (!instance) {
    return (
      <div className="flex h-full items-center justify-center">
        <Button variant="ghost" onClick={close}>
          <ArrowLeft className="h-4 w-4" /> Back
        </Button>
      </div>
    );
  }

  const exportInstance = async () => {
    try {
      const path = await save({
        defaultPath: `${instance.name}.zip`,
        filters: [{ name: "Zip archive", extensions: ["zip"] }],
      });
      if (!path) return;
      setExporting(true);
      toast("info", "Exporting instance…");
      await api.exportInstance(id, path);
      toast("success", "Instance exported");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setExporting(false);
    }
  };

  const tabs: { id: Tab; label: string; icon: typeof Package }[] = [
    { id: "content", label: "Content", icon: Package },
    { id: "logs", label: "Logs", icon: ScrollText },
    { id: "settings", label: "Settings", icon: Settings2 },
  ];

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="px-8 pt-6">
        <button
          onClick={close}
          className="mb-4 inline-flex items-center gap-1.5 text-sm text-muted-foreground transition hover:text-foreground btn-focus"
        >
          <ArrowLeft className="h-4 w-4" /> All instances
        </button>

        <div className="flex items-center gap-4">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-accent/25 to-accent/5 text-3xl">
            {instance.icon ?? instance.name.charAt(0).toUpperCase()}
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-2xl font-bold tracking-tight">{instance.name}</h1>
            <div className="mt-0.5 flex items-center gap-2 text-sm text-muted-foreground">
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
          <div className="flex items-center gap-2">
            <Button variant="secondary" onClick={exportInstance} loading={exporting} title="Export as .zip">
              <Upload className="h-4 w-4" /> Export
            </Button>
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
        <nav className="mt-6 flex gap-1 border-b border-border">
          {tabs.map((t) => {
            const Icon = t.icon;
            const active = tab === t.id;
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={cn(
                  "relative flex items-center gap-2 px-4 py-2.5 text-sm font-medium transition btn-focus",
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

      <div className="scroll-area flex-1 px-8 py-5">
        {tab === "content" && <ContentTab instance={instance} />}
        {tab === "logs" && <LogsTab id={id} />}
        {tab === "settings" && <SettingsTab instance={instance} />}
      </div>
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

// Run a provider-aware install; returns the installed filename.
function installProvider(
  provider: Provider,
  args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  },
) {
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

// --------------------------------------------------------------------------
// Mods panel (has enable/disable toggle)
// --------------------------------------------------------------------------

function ModsPanel({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const [installed, setInstalled] = useState<ModEntry[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [page, setPage] = useState(0);
  const [totalHits, setTotalHits] = useState(0);

  const refresh = () => api.listMods(instance.id).then(setInstalled).catch(() => {});
  useEffect(() => {
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

  const [versionPickTarget, setVersionPickTarget] = useState<ModHit | null>(null);

  const installWithVersion = async (hit: ModHit, versionId: string) => {
    setInstalling(hit.project_id);
    setVersionPickTarget(null);
    try {
      const file =
        provider === "curseforge"
          ? await api.installCurseforgeFile({
              instanceId: instance.id,
              projectId: hit.project_id,
              fileId: versionId,
              contentType: "mod",
            })
          : await api.installContentVersion({
              instanceId: instance.id,
              projectId: hit.project_id,
              versionId,
              contentType: "mod",
            });
      toast("success", `Installed ${file}`);
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

  const [updates, setUpdates] = useState<ModUpdate[]>([]);
  const [checking, setChecking] = useState(false);
  const [updatingAll, setUpdatingAll] = useState(false);

  const checkUpdates = async () => {
    setChecking(true);
    try {
      const u = await api.checkModUpdates({
        instanceId: instance.id,
        loader: instance.loader,
        gameVersion: instance.mcVersion,
      });
      setUpdates(u);
      toast(
        u.length ? "info" : "success",
        u.length ? `${u.length} update${u.length > 1 ? "s" : ""} available` : "All mods up to date",
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
      toast("success", `Updated ${updates.length} mod${updates.length > 1 ? "s" : ""}`);
      setUpdates([]);
      refresh();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setUpdatingAll(false);
    }
  };

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      {/* Installed */}
      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Installed ({installed.length})
          </h2>
          {installed.length > 0 && (
            <button
              onClick={checkUpdates}
              disabled={checking}
              className="inline-flex items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus disabled:opacity-50"
            >
              <RefreshCw className={cn("h-3 w-3", checking && "animate-spin")} />
              {checking ? "Checking…" : "Updates"}
            </button>
          )}
        </div>

        {updates.length > 0 && (
          <div className="mb-3 rounded-lg border border-accent/40 bg-accent/10 p-3">
            <p className="text-sm font-medium">
              {updates.length} update{updates.length > 1 ? "s" : ""} available
            </p>
            <ul className="mt-1.5 space-y-0.5 text-xs text-muted-foreground">
              {updates.slice(0, 4).map((u) => (
                <li key={u.oldFileName} className="truncate">
                  {u.newFileName.replace(/\.jar$/, "")}
                </li>
              ))}
              {updates.length > 4 && <li>+{updates.length - 4} more…</li>}
            </ul>
            <Button
              size="sm"
              variant="primary"
              className="mt-2 w-full"
              loading={updatingAll}
              onClick={applyAllUpdates}
            >
              Update all
            </Button>
          </div>
        )}
        {installed.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No mods yet. Search on the right to add some.
          </p>
        ) : (
          <div className="space-y-1.5">
            {installed.map((m) => (
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
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [provider, setProvider] = useState<Provider>("modrinth");
  const [page, setPage] = useState(0);
  const [totalHits, setTotalHits] = useState(0);

  const refresh = () => listItems().then(setInstalled).catch(() => {});
  useEffect(() => {
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

  const install = async (hit: ModHit) => {
    setInstalling(hit.project_id);
    try {
      const file = await installProvider(provider, {
        instanceId: instance.id,
        projectId: hit.project_id,
        contentType,
        loader: useLoaderFilter ? instance.loader : undefined,
        gameVersion: instance.mcVersion,
      });
      toast("success", `Installed ${file}`);
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

  const FallbackIcon =
    contentType === "shader" ? (
      <Sparkles className="h-5 w-5 text-muted-foreground" />
    ) : (
      <Image className="h-5 w-5 text-muted-foreground" />
    );

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[300px_minmax(0,1fr)]">
      {/* Installed */}
      <section>
        <h2 className="mb-3 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Installed ({installed.length})
        </h2>
        {installed.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No {emptyLabel} yet. Search on the right to add some.
          </p>
        ) : (
          <div className="space-y-1.5">
            {installed.map((item) => (
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
          onInstall={install}
          provider={provider}
          emptyLabel={emptyLabel}
          fallbackIcon={FallbackIcon}
          installedIds={
            new Set(installed.map((m) => m.projectId).filter((x): x is string => !!x))
          }
        />
        <Pagination page={page} pageCount={pageCountFor(totalHits)} onPage={setPage} />
      </section>
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
      <div className="flex justify-center py-10">
        <Spinner />
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

function VersionPickerModal({
  hit,
  provider,
  onClose,
  onInstall,
}: {
  hit: ModHit | null;
  provider: Provider;
  onClose: () => void;
  onInstall: (hit: ModHit, versionId: string) => void;
}) {
  const toast = useStore((s) => s.toast);
  const [versions, setVersions] = useState<ContentVersion[]>([]);
  const [versionId, setVersionId] = useState("");
  const [loading, setLoading] = useState(false);

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
      .then((v) => {
        setVersions(v);
        setVersionId(v[0]?.id ?? "");
      })
      .catch((e) => toast("error", errMessage(e)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hit?.project_id, provider]);

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
              <p className="text-sm text-muted-foreground">No compatible versions found.</p>
            ) : (
              <Select value={versionId} onChange={(e) => setVersionId(e.target.value)}>
                {versions.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name || v.versionNumber}
                    {v.gameVersions.length > 0 && ` — MC ${v.gameVersions[0]}`}
                    {` (${new Date(v.date).toLocaleDateString()})`}
                  </option>
                ))}
              </Select>
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
  const [worlds, setWorlds] = useState<WorldEntry[]>([]);

  const refresh = () =>
    api.listWorlds(instance.id).then(setWorlds).catch(() => {});
  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  const remove = async (name: string) => {
    if (!confirm(`Delete world "${name}"? This cannot be undone.`)) return;
    await api.deleteWorld(instance.id, name).catch((e) => toast("error", errMessage(e)));
    refresh();
  };

  if (worlds.length === 0) {
    return (
      <EmptyState
        icon={<Globe className="h-7 w-7" />}
        title="No worlds"
        description="Worlds will appear here once you play and create or load one."
      />
    );
  }

  return (
    <div className="max-w-xl space-y-1.5">
      {worlds.map((w) => (
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
      ))}
    </div>
  );
}

// --------------------------------------------------------------------------
// Screenshots panel
// --------------------------------------------------------------------------

function ScreenshotsPanel({ instance }: { instance: Instance }) {
  const [screenshots, setScreenshots] = useState<ScreenshotEntry[]>([]);

  useEffect(() => {
    api.listScreenshots(instance.id).then(setScreenshots).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

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
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-4">
      {screenshots.map((s) => (
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
  );
}

// --------------------------------------------------------------------------
// Logs
// --------------------------------------------------------------------------

function LogsTab({ id }: { id: string }) {
  const logs = useStore((s) => s.logs[id] ?? []);
  const clearLogs = useStore((s) => s.clearLogs);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs.length]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-sm text-muted-foreground">{logs.length} lines</span>
        <Button size="sm" variant="ghost" onClick={() => clearLogs(id)}>
          Clear
        </Button>
      </div>
      <div className="scroll-area flex-1 rounded-xl border bg-[hsl(240_12%_4%)] p-3 font-mono text-xs leading-relaxed">
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
  const [memory, setMemory] = useState(instance.memoryMb ?? 0);
  const [javaPath, setJavaPath] = useState(instance.javaPath ?? "");
  const [jvmArgs, setJvmArgs] = useState(instance.jvmArgs ?? "");
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    try {
      await update({
        ...instance,
        name: name.trim() || instance.name,
        group: group.trim() || null,
        memoryMb: memory > 0 ? memory : null,
        javaPath: javaPath.trim() || null,
        jvmArgs: jvmArgs.trim() || null,
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
        <textarea
          value={jvmArgs}
          onChange={(e) => setJvmArgs(e.target.value)}
          rows={3}
          placeholder="Leave empty to use the global JVM arguments."
          className="input-base resize-none font-mono text-xs"
        />
      </Field>

      <div className="flex justify-end">
        <Button variant="primary" onClick={save} loading={saving}>
          Save changes
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

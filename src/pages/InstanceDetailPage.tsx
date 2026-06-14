import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  Download,
  FolderOpen,
  Package,
  Play,
  ScrollText,
  Settings2,
  Square,
  Trash2,
} from "lucide-react";
import { Button, EmptyState, Field, Input, Spinner } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import { cn, formatNumber, loaderLabel } from "@/lib/utils";
import type { Instance, ModEntry, ModHit } from "@/lib/types";

type Tab = "content" | "logs" | "settings";

export function InstanceDetailPage({ id }: { id: string }) {
  const instance = useStore((s) => s.instances.find((i) => i.id === id));
  const running = useStore((s) => s.running.has(id));
  const launch = useStore((s) => s.launch);
  const stop = useStore((s) => s.stop);
  const close = useStore((s) => s.closeInstance);
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
// Content / mods
// --------------------------------------------------------------------------

function ContentTab({ instance }: { instance: Instance }) {
  const toast = useStore((s) => s.toast);
  const [installed, setInstalled] = useState<ModEntry[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);

  const isModded = instance.loader === "fabric" || instance.loader === "quilt";

  const refreshInstalled = () => api.listMods(instance.id).then(setInstalled).catch(() => {});
  useEffect(() => {
    refreshInstalled();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance.id]);

  // Debounced Modrinth search.
  useEffect(() => {
    if (!isModded) return;
    setSearching(true);
    const t = setTimeout(() => {
      api
        .searchModrinth({
          query,
          projectType: "mod",
          loader: instance.loader,
          gameVersion: instance.mcVersion,
          limit: 20,
        })
        .then((r) => setResults(r.hits))
        .catch((e) => toast("error", errMessage(e)))
        .finally(() => setSearching(false));
    }, 350);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, isModded, instance.loader, instance.mcVersion]);

  const install = async (hit: ModHit) => {
    setInstalling(hit.project_id);
    try {
      const file = await api.installMod({
        instanceId: instance.id,
        projectId: hit.project_id,
        loader: instance.loader,
        gameVersion: instance.mcVersion,
      });
      toast("success", `Installed ${file}`);
      refreshInstalled();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setInstalling(null);
    }
  };

  const toggle = async (m: ModEntry) => {
    await api.setModEnabled(instance.id, m.fileName, !m.enabled).catch((e) => toast("error", errMessage(e)));
    refreshInstalled();
  };
  const remove = async (m: ModEntry) => {
    await api.deleteMod(instance.id, m.fileName).catch((e) => toast("error", errMessage(e)));
    refreshInstalled();
  };

  if (!isModded) {
    return (
      <EmptyState
        icon={<Package className="h-7 w-7" />}
        title="No mod loader"
        description="This is a Vanilla instance. Create a Fabric or Quilt instance to browse and install mods from Modrinth."
      />
    );
  }

  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-[320px_1fr]">
      {/* Installed */}
      <section>
        <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-muted-foreground">
          Installed ({installed.length})
        </h2>
        {installed.length === 0 ? (
          <p className="rounded-lg border border-dashed p-4 text-sm text-muted-foreground">
            No mods installed yet. Search and install from the right.
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
                <span
                  className={cn(
                    "flex-1 truncate text-sm",
                    !m.enabled && "text-muted-foreground line-through",
                  )}
                  title={m.fileName}
                >
                  {m.fileName.replace(/\.jar$/, "")}
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
        <div className="mb-3">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search Modrinth for ${instance.mcVersion} ${loaderLabel(instance.loader)} mods…`}
          />
        </div>
        {searching && results.length === 0 ? (
          <div className="flex justify-center py-10">
            <Spinner />
          </div>
        ) : (
          <div className="space-y-2">
            {results.map((hit) => {
              const isInstalling = installing === hit.project_id;
              return (
                <div
                  key={hit.project_id}
                  className="flex items-center gap-3 rounded-xl border bg-card/60 p-3 transition hover:bg-card"
                >
                  {hit.icon_url ? (
                    <img
                      src={hit.icon_url}
                      alt=""
                      className="h-11 w-11 shrink-0 rounded-lg object-cover"
                    />
                  ) : (
                    <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-muted">
                      <Package className="h-5 w-5 text-muted-foreground" />
                    </div>
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate font-medium">{hit.title}</span>
                      <span className="shrink-0 text-xs text-muted-foreground">
                        {formatNumber(hit.downloads)} ↓
                      </span>
                    </div>
                    <p className="truncate text-sm text-muted-foreground">{hit.description}</p>
                  </div>
                  <Button
                    size="sm"
                    variant="secondary"
                    loading={isInstalling}
                    onClick={() => install(hit)}
                  >
                    {!isInstalling && <Download className="h-3.5 w-3.5" />}
                    Add
                  </Button>
                </div>
              );
            })}
            {!searching && results.length === 0 && (
              <p className="py-10 text-center text-sm text-muted-foreground">No mods found.</p>
            )}
          </div>
        )}
      </section>
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

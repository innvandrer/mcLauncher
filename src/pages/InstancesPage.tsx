import { useMemo, useState } from "react";
import {
  Archive,
  Boxes,
  CheckSquare2,
  ChevronDown,
  Grid2X2,
  Link2,
  List,
  Plus,
  Search,
  Tag,
  Trash2,
  X,
} from "lucide-react";
import { Button, Input, Select } from "@/components/ui";
import { InstanceCard } from "@/components/InstanceCard";
import { InstanceIcon } from "@/components/InstanceIcon";
import { CreateInstanceModal } from "@/components/CreateInstanceModal";
import { useStore } from "@/store/useStore";
import { cn, LOADERS, loaderLabel, timeAgo } from "@/lib/utils";
import { groupInstances, type LoaderFilter } from "@/lib/instances";
import { t } from "@/lib/strings";
import type { Instance } from "@/lib/types";
import { api, errMessage } from "@/lib/api";

type SortKey = "recent" | "name" | "playtime" | "mods";

export function InstancesPage() {
  const instances = useStore((s) => s.instances);
  const openInstance = useStore((s) => s.openInstance);
  const updateInstance = useStore((s) => s.updateInstance);
  const deleteInstance = useStore((s) => s.deleteInstance);
  const toast = useStore((s) => s.toast);
  const [query, setQuery] = useState("");
  const [loaderFilter, setLoaderFilter] = useState<LoaderFilter>("all");
  const [createOpen, setCreateOpen] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [view, setView] = useState<"grid" | "list">(
    () =>
      (localStorage.getItem("ezmapa:library-view") as "grid" | "list") ||
      "grid",
  );
  const [sort, setSort] = useState<SortKey>("recent");
  const [showArchived, setShowArchived] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const sorted = useMemo(
    () =>
      [...instances]
        .filter((item) => !!item.archived === showArchived)
        .sort((a, b) =>
          sort === "name"
            ? a.name.localeCompare(b.name)
            : sort === "playtime"
              ? b.totalPlaySeconds - a.totalPlaySeconds
              : sort === "mods"
                ? (b.modCount ?? 0) - (a.modCount ?? 0)
                : (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0),
        ),
    [instances, showArchived, sort],
  );
  const groups = useMemo(
    () => groupInstances(sorted, query, loaderFilter),
    [sorted, query, loaderFilter],
  );
  const matchCount = groups.reduce(
    (count, group) => count + group.instances.length,
    0,
  );
  const chooseView = (next: "grid" | "list") => {
    setView(next);
    localStorage.setItem("ezmapa:library-view", next);
  };
  const toggleSelected = (id: string) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const bulkUpdate = async (change: Partial<Instance>) => {
    await Promise.all(
      [...selected].map((id) => {
        const item = instances.find((candidate) => candidate.id === id);
        return item
          ? updateInstance({ ...item, ...change })
          : Promise.resolve();
      }),
    );
    toast("success", `Updated ${selected.size} instances`);
    setSelected(new Set());
  };
  const bulkDelete = async () => {
    if (
      !globalThis.confirm(
        `Delete ${selected.size} selected instances and their files?`,
      )
    )
      return;
    for (const id of selected) await deleteInstance(id);
    setSelected(new Set());
  };
  const bulkGroup = async () => {
    const group = globalThis.prompt("Move selected instances to group:", "");
    if (group === null) return;
    await bulkUpdate({ group: group.trim() || null });
  };
  const importShareCode = async () => {
    const encoded = globalThis.prompt(
      "Paste an EZMapa Pass the Pack code:",
      "",
    );
    if (!encoded) return;
    try {
      const raw = encoded.trim().replace(/^EZMAPA1:/, "");
      const binary = atob(raw);
      const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
      const instance = await api.importShareCode(
        new TextDecoder().decode(bytes),
      );
      await useStore.getState().refreshInstances();
      toast("success", `Imported “${instance.name}”`);
      openInstance(instance.id);
    } catch (error) {
      toast("error", `Could not import share code: ${errMessage(error)}`);
    }
  };
  const dropOnGroup = async (event: React.DragEvent, group: string) => {
    event.preventDefault();
    const id = event.dataTransfer.getData("text/ezmapa-instance");
    const item = instances.find((candidate) => candidate.id === id);
    if (item) await updateInstance({ ...item, group: group || null });
  };
  const toggleGroup = (key: string) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  return (
    <div className="app-page">
      <header className="app-gutter pt-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">
              {t("instances.title")}
            </h1>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {showArchived
                ? "Archived setups stay out of the way without being deleted."
                : "Organize, search, and launch every Minecraft setup."}
            </p>
          </div>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={importShareCode}>
              <Link2 className="h-4 w-4" />
              Import code
            </Button>
            <Button variant="primary" onClick={() => setCreateOpen(true)}>
              <Plus className="h-4 w-4" />
              {t("instances.create")}
            </Button>
          </div>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <div className="relative min-w-[220px] flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search names, tags, versions, groups…"
              className="pl-9"
            />
          </div>
          <Select
            value={sort}
            onChange={(e) => setSort(e.target.value as SortKey)}
            className="w-36"
          >
            <option value="recent">Recent</option>
            <option value="name">Name</option>
            <option value="playtime">Playtime</option>
            <option value="mods">Mod count</option>
          </Select>
          <button
            onClick={() => setShowArchived((value) => !value)}
            className={cn(
              "flex h-9 items-center gap-2 rounded-lg border px-3 text-xs font-medium",
              showArchived
                ? "border-accent/40 bg-accent/10 text-accent"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Archive className="h-3.5 w-3.5" />
            Archived
          </button>
          <div className="inline-flex rounded-lg border p-0.5">
            <button
              onClick={() => chooseView("grid")}
              className={cn(
                "rounded-md p-2",
                view === "grid"
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground",
              )}
              title="Grid"
            >
              <Grid2X2 className="h-3.5 w-3.5" />
            </button>
            <button
              onClick={() => chooseView("list")}
              className={cn(
                "rounded-md p-2",
                view === "list"
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground",
              )}
              title="List"
            >
              <List className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
        <div className="mt-2 flex gap-1 overflow-x-auto">
          <FilterChip
            active={loaderFilter === "all"}
            onClick={() => setLoaderFilter("all")}
          >
            All loaders
          </FilterChip>
          {LOADERS.map((loader) => (
            <FilterChip
              key={loader.id}
              active={loaderFilter === loader.id}
              onClick={() => setLoaderFilter(loader.id)}
            >
              {loader.label}
            </FilterChip>
          ))}
        </div>
      </header>

      <div className="app-scroll app-gutter py-5">
        {instances.length === 0 ? (
          <Empty onCreate={() => setCreateOpen(true)} />
        ) : matchCount === 0 ? (
          <div className="rounded-2xl border border-dashed py-16 text-center">
            <Search className="mx-auto mb-3 h-7 w-7 text-muted-foreground" />
            <p className="text-sm font-medium">No matching instances</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Try another name, tag, version, or loader.
            </p>
          </div>
        ) : (
          <div className="space-y-6">
            {groups.map((group) => {
              const isCollapsed = collapsed.has(group.key) && !query.trim();
              return (
                <section
                  key={group.key}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => dropOnGroup(e, group.label)}
                  className="rounded-xl transition hover:bg-muted/[.04]"
                >
                  <button
                    type="button"
                    onClick={() => toggleGroup(group.key)}
                    className="mb-3 flex w-full items-center gap-2 rounded-md text-left btn-focus"
                    aria-expanded={!isCollapsed}
                  >
                    <ChevronDown
                      className={cn(
                        "h-4 w-4 text-muted-foreground transition-transform",
                        isCollapsed && "-rotate-90",
                      )}
                    />
                    <h2 className="text-sm font-semibold">
                      {group.label || t("instances.ungrouped")}
                    </h2>
                    <span className="text-xs text-muted-foreground">
                      {group.instances.length}
                    </span>
                    <span className="ml-auto text-[11px] text-muted-foreground opacity-0 transition group-hover:opacity-100">
                      Drop here to regroup
                    </span>
                  </button>
                  {!isCollapsed && (
                    <div
                      className={
                        view === "grid"
                          ? "grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4"
                          : "space-y-2"
                      }
                    >
                      {group.instances.map((instance) => (
                        <div
                          key={instance.id}
                          draggable
                          onDragStart={(e) =>
                            e.dataTransfer.setData(
                              "text/ezmapa-instance",
                              instance.id,
                            )
                          }
                          className="relative"
                        >
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              toggleSelected(instance.id);
                            }}
                            className={cn(
                              "absolute left-2 top-2 z-20 flex h-6 w-6 items-center justify-center rounded-md border shadow",
                              selected.has(instance.id)
                                ? "border-accent bg-accent text-accent-foreground"
                                : "bg-background/85 text-transparent hover:text-muted-foreground",
                            )}
                            aria-label={`Select ${instance.name}`}
                          >
                            <CheckSquare2 className="h-3.5 w-3.5" />
                          </button>
                          {view === "grid" ? (
                            <InstanceCard
                              instance={instance}
                              onOpen={() => openInstance(instance.id)}
                            />
                          ) : (
                            <InstanceRow
                              instance={instance}
                              onOpen={() => openInstance(instance.id)}
                            />
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </section>
              );
            })}
          </div>
        )}
      </div>

      {selected.size > 0 && (
        <div className="fixed bottom-5 left-1/2 z-40 flex -translate-x-1/2 items-center gap-1 rounded-2xl border bg-background/95 p-2 shadow-2xl glass">
          <span className="px-2 text-sm font-semibold">
            {selected.size} selected
          </span>
          <Button size="sm" variant="ghost" onClick={bulkGroup}>
            <Tag className="h-4 w-4" /> Group
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => bulkUpdate({ archived: !showArchived })}
          >
            <Archive className="h-4 w-4" />
            {showArchived ? "Restore" : "Archive"}
          </Button>
          <Button size="sm" variant="ghost" onClick={bulkDelete}>
            <Trash2 className="h-4 w-4" /> Delete
          </Button>
          <button
            onClick={() => setSelected(new Set())}
            className="rounded-lg p-2 hover:bg-muted"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      )}
      <CreateInstanceModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
      />
    </div>
  );
}

function InstanceRow({
  instance,
  onOpen,
}: {
  instance: Instance;
  onOpen: () => void;
}) {
  const running = useStore((s) => s.running.has(instance.id));
  const launch = useStore((s) => s.launch);
  return (
    <button
      onClick={onOpen}
      className="flex w-full items-center gap-4 rounded-xl border bg-card/55 py-3 pl-11 pr-3 text-left transition hover:border-accent/25 hover:bg-muted/30"
    >
      <InstanceIcon instance={instance} size="sm" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-semibold">
            {instance.name}
          </span>
          {instance.tags?.slice(0, 2).map((tag) => (
            <span
              key={tag}
              className="rounded bg-accent/10 px-1.5 py-0.5 text-[10px] text-accent"
            >
              {tag}
            </span>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          {instance.mcVersion} · {loaderLabel(instance.loader)} ·{" "}
          {instance.modCount ?? 0} mods
        </p>
      </div>
      <span className="hidden text-xs text-muted-foreground sm:block">
        {instance.lastPlayed ? timeAgo(instance.lastPlayed) : "Never played"}
      </span>
      <Button
        size="sm"
        variant={running ? "secondary" : "primary"}
        onClick={(e) => {
          e.stopPropagation();
          if (!running) launch(instance.id);
        }}
      >
        <span className="sr-only">Play</span>
        {running ? "Running" : "Play"}
      </Button>
    </button>
  );
}
function FilterChip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "shrink-0 rounded-md px-2.5 py-1.5 text-xs font-medium transition btn-focus",
        active
          ? "bg-accent/20 text-accent"
          : "text-muted-foreground hover:bg-muted hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}
function Empty({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center gap-4 rounded-2xl border border-dashed py-16 text-center text-muted-foreground">
      <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/60">
        <Boxes className="h-7 w-7" />
      </div>
      <div>
        <p className="font-medium text-foreground">
          Build your first Minecraft setup
        </p>
        <p className="mt-1 max-w-sm text-sm">
          Choose a game version and loader, then add content when you’re ready.
        </p>
      </div>
      <Button variant="primary" onClick={onCreate}>
        <Plus className="h-4 w-4" />
        Create instance
      </Button>
    </div>
  );
}

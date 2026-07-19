import { useEffect, useState } from "react";
import {
  Activity,
  ArchiveRestore,
  Camera,
  Clock3,
  Gauge,
  Globe2,
  HardDrive,
  HeartPulse,
  Image,
  Layers3,
  Play,
  Plus,
  Rocket,
  Server,
  ShieldCheck,
  Sparkles,
  Trash2,
  WifiOff,
} from "lucide-react";
import { api, errMessage } from "@/lib/api";
import { useActivities } from "@/lib/activity";
import { formatBytes, cn } from "@/lib/utils";
import { useStore } from "@/store/useStore";
import { Button, Field, Input, Modal, Select, Spinner } from "@/components/ui";
import type {
  DiskUsage,
  Instance,
  LaunchProfile,
  OfflineReadiness,
  PreflightWarning,
  SavedServer,
  ScreenshotEntry,
  Snapshot,
  StartupStats,
  WorldEntry,
} from "@/lib/types";

export function InstanceOverview({
  instance,
  onHealth,
  onDoctor,
  onOpenTab,
}: {
  instance: Instance;
  onHealth: () => void;
  onDoctor: () => void;
  onOpenTab: (tab: "content" | "servers" | "logs" | "settings") => void;
}) {
  const launch = useStore((s) => s.launch);
  const running = useStore((s) => s.running.has(instance.id));
  const [loading, setLoading] = useState(true);
  const [worlds, setWorlds] = useState<WorldEntry[]>([]);
  const [servers, setServers] = useState<SavedServer[]>([]);
  const [screens, setScreens] = useState<ScreenshotEntry[]>([]);
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [disk, setDisk] = useState<DiskUsage | null>(null);
  const [stats, setStats] = useState<StartupStats | null>(null);
  const [warnings, setWarnings] = useState<PreflightWarning[]>([]);
  const [profilesOpen, setProfilesOpen] = useState(false);
  const [offline, setOffline] = useState<OfflineReadiness | null>(null);
  const activity = useActivities()
    .filter((entry) => entry.instanceId === instance.id || !entry.instanceId)
    .slice(0, 5);

  useEffect(() => {
    let current = true;
    Promise.all([
      api.listWorlds(instance.id),
      api.listServers(instance.id),
      api.listScreenshots(instance.id),
      api.listSnapshots(instance.id),
      api.instanceDiskUsage(instance.id),
      api.startupStats(instance.id),
      api.preflightCheck(instance.id),
      api.offlineReadiness(instance.id),
    ])
      .then(([w, s, shots, snaps, usage, startup, health, readiness]) => {
        if (!current) return;
        setWorlds(w);
        setServers(s);
        setScreens(shots);
        setSnapshots(snaps);
        setDisk(usage);
        setStats(startup);
        setWarnings(health);
        setOffline(readiness);
      })
      .catch(() => {})
      .finally(() => current && setLoading(false));
    return () => {
      current = false;
    };
  }, [instance.id]);

  const latestWorld = worlds[0];
  const latestServer = servers[0];
  const recentDestination = latestWorld
    ? { world: latestWorld.name }
    : latestServer
      ? { server: latestServer.ip }
      : undefined;
  const totalDisk = disk
    ? Object.values(disk)
        .filter((value): value is number => typeof value === "number")
        .reduce((a, b) => a + b, 0)
    : 0;

  if (loading)
    return (
      <div className="flex items-center justify-center py-20 text-muted-foreground">
        <Spinner className="mr-2 h-5 w-5" /> Building command center…
      </div>
    );

  return (
    <div className="space-y-5">
      <section className="relative overflow-hidden rounded-2xl border bg-gradient-to-br from-accent/15 via-card to-card p-5 sm:p-6">
        <div className="pointer-events-none absolute -right-16 -top-16 h-52 w-52 rounded-full bg-accent/10 blur-3xl" />
        <div className="relative flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <div className="mb-2 flex flex-wrap items-center gap-2 text-xs font-medium">
              <span
                className={cn(
                  "rounded-full px-2.5 py-1",
                  warnings.length
                    ? "bg-amber-500/15 text-amber-400"
                    : "bg-emerald-500/15 text-emerald-400",
                )}
              >
                {warnings.length
                  ? `${warnings.length} health item${warnings.length === 1 ? "" : "s"}`
                  : "Ready to play"}
              </span>
              <span className="rounded-full bg-muted px-2.5 py-1 text-muted-foreground">
                {instance.modCount ?? 0} mods
              </span>
              <span className="rounded-full bg-muted px-2.5 py-1 text-muted-foreground">
                {formatBytes(totalDisk)}
              </span>
            </div>
            <h2 className="text-xl font-bold">Continue your adventure</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {latestWorld
                ? `Jump back into ${latestWorld.name}`
                : latestServer
                  ? `Reconnect to ${latestServer.name}`
                  : "Start Minecraft with your current loadout."}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" onClick={() => setProfilesOpen(true)}>
              <Rocket className="h-4 w-4" /> Launch profiles
            </Button>
            <Button
              variant="primary"
              disabled={running}
              onClick={() =>
                recentDestination
                  ? api
                      .launchInstance(instance.id, recentDestination)
                      .catch(() => launch(instance.id))
                  : launch(instance.id)
              }
            >
              <Play className="h-4 w-4 fill-current" />{" "}
              {running
                ? "Running"
                : latestWorld || latestServer
                  ? "Quick play"
                  : "Play"}
            </Button>
          </div>
        </div>
      </section>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <Metric
          icon={warnings.length ? HeartPulse : ShieldCheck}
          label="Pack health"
          value={warnings.length ? `${warnings.length} to review` : "Healthy"}
          tone={warnings.length ? "warn" : "good"}
          onClick={onHealth}
        />
        <Metric
          icon={Clock3}
          label="Average startup"
          value={
            stats?.current?.avgMs
              ? `${(stats.current.avgMs / 1000).toFixed(1)} sec`
              : "Not measured"
          }
        />
        <Metric
          icon={ArchiveRestore}
          label="World backups"
          value={`${snapshots.length} available`}
          onClick={() => onOpenTab("content")}
        />
        <Metric
          icon={HardDrive}
          label="Instance size"
          value={formatBytes(totalDisk)}
          onClick={() => onOpenTab("settings")}
        />
      </div>

      <div className="grid gap-5 xl:grid-cols-[1.35fr_1fr]">
        <section className="rounded-2xl border bg-card/60 p-4">
          <div className="mb-3 flex items-center justify-between">
            <div>
              <h3 className="font-semibold">Jump back in</h3>
              <p className="text-xs text-muted-foreground">
                Worlds and servers available for Smart Quick Play
              </p>
            </div>
            <Sparkles className="h-4 w-4 text-accent" />
          </div>
          <div className="grid gap-2 sm:grid-cols-2">
            {worlds.slice(0, 3).map((world) => (
              <QuickDestination
                key={world.name}
                icon={Globe2}
                title={world.name}
                subtitle={
                  world.modified
                    ? `Played ${new Date(world.modified * 1000).toLocaleDateString()} · ${formatBytes(world.size)}`
                    : formatBytes(world.size)
                }
                onClick={() =>
                  api.launchInstance(instance.id, { world: world.name })
                }
              />
            ))}
            {servers.slice(0, 3).map((server) => (
              <QuickDestination
                key={server.ip}
                icon={Server}
                title={server.name}
                subtitle={server.ip}
                onClick={() =>
                  api.launchInstance(instance.id, { server: server.ip })
                }
              />
            ))}
            {!worlds.length && !servers.length && (
              <button
                onClick={() => onOpenTab("servers")}
                className="col-span-full rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground hover:border-accent/50 hover:text-foreground"
              >
                <Plus className="mx-auto mb-2 h-5 w-5" />
                Add a server or create a world in game
              </button>
            )}
          </div>
        </section>

        <section className="rounded-2xl border bg-card/60 p-4">
          <div className="mb-3 flex items-center justify-between">
            <div>
              <h3 className="font-semibold">Recent activity</h3>
              <p className="text-xs text-muted-foreground">
                What changed in this setup
              </p>
            </div>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </div>
          {activity.length ? (
            <div className="space-y-1">
              {activity.map((entry) => (
                <div
                  key={entry.id}
                  className="flex gap-3 rounded-lg px-2 py-2 hover:bg-muted/50"
                >
                  <div className="mt-1 h-2 w-2 shrink-0 rounded-full bg-accent" />
                  <div className="min-w-0">
                    <p className="truncate text-sm">{entry.message}</p>
                    <p className="text-[11px] text-muted-foreground">
                      {new Date(entry.created).toLocaleString()}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="py-8 text-center text-sm text-muted-foreground">
              Install, update, or launch something to begin the timeline.
            </div>
          )}
        </section>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <ActionCard
          icon={Layers3}
          title="Pack Doctor"
          detail="Resume a guided conflict search and isolate unstable mods."
          onClick={onDoctor}
        />
        <ActionCard
          icon={WifiOff}
          title={
            offline?.ready
              ? "Ready for offline play"
              : "Offline setup incomplete"
          }
          detail={
            offline?.ready
              ? "Version files, libraries, assets, and Java are cached locally."
              : offline?.missing.length
                ? `Missing ${offline.missing.join(", ")}. Launch once while online to prepare.`
                : "Check locally cached game files."
          }
          onClick={() => api.offlineReadiness(instance.id).then(setOffline)}
        />
        <ActionCard
          icon={Camera}
          title="Screenshot studio"
          detail={`${screens.length} screenshot${screens.length === 1 ? "" : "s"} ready to browse and share.`}
          onClick={() => onOpenTab("content")}
        />
      </div>
      <LaunchProfilesModal
        open={profilesOpen}
        onClose={() => setProfilesOpen(false)}
        instance={instance}
      />
    </div>
  );
}

function Metric({
  icon: Icon,
  label,
  value,
  tone,
  onClick,
}: {
  icon: typeof Gauge;
  label: string;
  value: string;
  tone?: "warn" | "good";
  onClick?: () => void;
}) {
  const content = (
    <>
      <div
        className={cn(
          "mb-3 flex h-9 w-9 items-center justify-center rounded-xl",
          tone === "warn"
            ? "bg-amber-500/15 text-amber-400"
            : tone === "good"
              ? "bg-emerald-500/15 text-emerald-400"
              : "bg-accent/12 text-accent",
        )}
      >
        <Icon className="h-4 w-4" />
      </div>
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-0.5 font-semibold">{value}</p>
    </>
  );
  return onClick ? (
    <button
      onClick={onClick}
      className="rounded-2xl border bg-card/60 p-4 text-left transition hover:-translate-y-0.5 hover:border-accent/30 btn-focus"
    >
      {content}
    </button>
  ) : (
    <div className="rounded-2xl border bg-card/60 p-4">{content}</div>
  );
}

function QuickDestination({
  icon: Icon,
  title,
  subtitle,
  onClick,
}: {
  icon: typeof Globe2;
  title: string;
  subtitle: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="group flex items-center gap-3 rounded-xl border bg-background/50 p-3 text-left transition hover:border-accent/40 hover:bg-accent/5"
    >
      <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted group-hover:bg-accent/15 group-hover:text-accent">
        <Icon className="h-4 w-4" />
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{title}</p>
        <p className="truncate text-xs text-muted-foreground">{subtitle}</p>
      </div>
      <Play className="h-3.5 w-3.5 opacity-0 transition group-hover:opacity-100" />
    </button>
  );
}
function ActionCard({
  icon: Icon,
  title,
  detail,
  onClick,
}: {
  icon: typeof Image;
  title: string;
  detail: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="rounded-2xl border bg-card/60 p-4 text-left transition hover:border-accent/30 hover:bg-accent/[.03]"
    >
      <Icon className="mb-3 h-5 w-5 text-accent" />
      <h3 className="text-sm font-semibold">{title}</h3>
      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
        {detail}
      </p>
    </button>
  );
}

function LaunchProfilesModal({
  open,
  onClose,
  instance,
}: {
  open: boolean;
  onClose: () => void;
  instance: Instance;
}) {
  const updateInstance = useStore((s) => s.updateInstance);
  const toast = useStore((s) => s.toast);
  const [name, setName] = useState("");
  const [memory, setMemory] = useState(String(instance.memoryMb ?? 4096));
  const [loadout, setLoadout] = useState("");
  const [world, setWorld] = useState("");
  const [server, setServer] = useState("");
  const [busy, setBusy] = useState(false);
  const profiles = instance.launchProfiles ?? [];
  const save = async () => {
    if (!name.trim()) return;
    setBusy(true);
    try {
      const profile: LaunchProfile = {
        name: name.trim(),
        memoryMb: Number(memory) || null,
        loadout: loadout || null,
        quickWorld: world.trim() || null,
        quickServer: server.trim() || null,
      };
      await updateInstance({
        ...instance,
        launchProfiles: [
          ...profiles.filter((p) => p.name !== profile.name),
          profile,
        ],
      });
      toast("success", `Saved launch profile “${profile.name}”`);
      setName("");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const play = async (profile: LaunchProfile) => {
    setBusy(true);
    try {
      if (profile.loadout) await api.applyLoadout(instance.id, profile.loadout);
      if (profile.memoryMb || profile.jvmArgs)
        await updateInstance({
          ...instance,
          memoryMb: profile.memoryMb ?? instance.memoryMb,
          jvmArgs: profile.jvmArgs ?? instance.jvmArgs,
        });
      await api.launchInstance(instance.id, {
        ...(profile.quickWorld ? { world: profile.quickWorld } : {}),
        ...(profile.quickServer ? { server: profile.quickServer } : {}),
      });
      onClose();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const remove = (profile: LaunchProfile) =>
    updateInstance({
      ...instance,
      launchProfiles: profiles.filter((p) => p.name !== profile.name),
    });
  return (
    <Modal open={open} onClose={onClose} title="Launch profiles">
      <div className="space-y-4">
        {profiles.map((profile) => (
          <div
            key={profile.name}
            className="flex items-center gap-3 rounded-xl border p-3"
          >
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-accent/15 text-accent">
              <Rocket className="h-4 w-4" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="font-medium">{profile.name}</p>
              <p className="truncate text-xs text-muted-foreground">
                {profile.memoryMb ? `${profile.memoryMb} MB` : "Default memory"}
                {profile.loadout ? ` · ${profile.loadout}` : ""}
                {profile.quickWorld
                  ? ` · ${profile.quickWorld}`
                  : profile.quickServer
                    ? ` · ${profile.quickServer}`
                    : ""}
              </p>
            </div>
            <Button
              size="sm"
              variant="primary"
              disabled={busy}
              onClick={() => play(profile)}
            >
              <Play className="h-3.5 w-3.5" /> Play
            </Button>
            <button
              onClick={() => remove(profile)}
              className="rounded-lg p-2 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            >
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        ))}
        {!profiles.length && (
          <div className="rounded-xl border border-dashed p-5 text-center text-sm text-muted-foreground">
            No profiles yet. Build one below.
          </div>
        )}
        <div className="grid gap-3 rounded-xl bg-muted/35 p-4 sm:grid-cols-2">
          <Field label="Profile name">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Speedrun, Creative…"
            />
          </Field>
          <Field label="Memory (MB)">
            <Input
              type="number"
              value={memory}
              onChange={(e) => setMemory(e.target.value)}
            />
          </Field>
          <Field label="Loadout">
            <Select
              value={loadout}
              onChange={(e) => setLoadout(e.target.value)}
            >
              <option value="">Current loadout</option>
              {(instance.loadouts ?? []).map((item) => (
                <option key={item.name}>{item.name}</option>
              ))}
            </Select>
          </Field>
          <Field label="Quick world">
            <Input
              value={world}
              onChange={(e) => setWorld(e.target.value)}
              placeholder="Optional world name"
            />
          </Field>
          <Field label="Quick server">
            <Input
              value={server}
              onChange={(e) => setServer(e.target.value)}
              placeholder="Optional address"
            />
          </Field>
          <div className="flex items-end">
            <Button
              variant="secondary"
              className="w-full"
              disabled={busy || !name.trim()}
              onClick={save}
            >
              <Plus className="h-4 w-4" /> Save profile
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}

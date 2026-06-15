import { useMemo } from "react";
import { motion } from "framer-motion";
import { Boxes, Clock, Package, Play, Trophy, Users } from "lucide-react";
import { useStore } from "@/store/useStore";
import { cn, formatDuration, loaderLabel, timeAgo } from "@/lib/utils";
import type { Instance, Loader } from "@/lib/types";

export function HomePage() {
  const instances = useStore((s) => s.instances);
  const accounts = useStore((s) => s.accounts);
  const running = useStore((s) => s.running);
  const openInstance = useStore((s) => s.openInstance);
  const setView = useStore((s) => s.setView);
  const launch = useStore((s) => s.launch);

  const stats = useMemo(() => {
    const totalSeconds = instances.reduce((a, i) => a + (i.totalPlaySeconds || 0), 0);
    const totalMods = instances.reduce((a, i) => a + (i.modCount || 0), 0);

    const byPlaytime = [...instances]
      .filter((i) => (i.totalPlaySeconds || 0) > 0)
      .sort((a, b) => b.totalPlaySeconds - a.totalPlaySeconds);

    const recent = [...instances]
      .filter((i) => i.lastPlayed)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
      .slice(0, 5);

    // Count instances per loader.
    const loaderCounts = new Map<Loader, number>();
    for (const i of instances) loaderCounts.set(i.loader, (loaderCounts.get(i.loader) ?? 0) + 1);
    const loaders = [...loaderCounts.entries()].sort((a, b) => b[1] - a[1]);

    return {
      totalSeconds,
      totalMods,
      byPlaytime,
      recent,
      loaders,
      mostPlayed: byPlaytime[0] ?? null,
      maxPlaytime: byPlaytime[0]?.totalPlaySeconds ?? 0,
    };
  }, [instances]);

  if (instances.length === 0) {
    return (
      <div className="flex h-full flex-col">
        <Header />
        <div className="flex flex-1 flex-col items-center justify-center gap-4 text-center text-muted-foreground">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/60">
            <Boxes className="h-7 w-7" />
          </div>
          <p className="max-w-sm text-sm">
            No instances yet. Create one to start tracking your playtime and stats.
          </p>
          <button
            onClick={() => setView("instances")}
            className="inline-flex h-10 items-center gap-2 rounded-lg bg-accent px-4 text-sm font-semibold text-accent-foreground shadow-lg shadow-accent/25 transition hover:brightness-110 btn-focus"
          >
            <Boxes className="h-4 w-4" />
            Go to instances
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <Header />

      <div className="scroll-area flex-1 px-8 pb-8">
        {/* Stat cards */}
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          <StatCard
            icon={<Clock className="h-5 w-5" />}
            label="Total playtime"
            value={stats.totalSeconds > 0 ? formatDuration(stats.totalSeconds) : "—"}
          />
          <StatCard
            icon={<Boxes className="h-5 w-5" />}
            label="Instances"
            value={`${instances.length}`}
          />
          <StatCard
            icon={<Package className="h-5 w-5" />}
            label="Mods installed"
            value={`${stats.totalMods}`}
          />
          <StatCard
            icon={<Users className="h-5 w-5" />}
            label="Accounts"
            value={`${accounts.length}`}
          />
        </div>

        {/* Most played hero */}
        {stats.mostPlayed && (
          <MostPlayedCard
            instance={stats.mostPlayed}
            running={running.has(stats.mostPlayed.id)}
            onOpen={() => openInstance(stats.mostPlayed!.id)}
            onPlay={() => launch(stats.mostPlayed!.id)}
          />
        )}

        <div className="mt-6 grid grid-cols-1 gap-6 lg:grid-cols-[1fr_300px]">
          {/* Top instances by playtime */}
          <section className="card-surface p-5">
            <h2 className="mb-4 flex items-center gap-2 text-sm font-semibold">
              <Trophy className="h-4 w-4 text-accent" />
              Top instances
            </h2>
            {stats.byPlaytime.length === 0 ? (
              <p className="py-6 text-center text-sm text-muted-foreground">
                No playtime recorded yet. Launch an instance to get started.
              </p>
            ) : (
              <div className="space-y-3">
                {stats.byPlaytime.slice(0, 6).map((inst) => (
                  <PlaytimeBar
                    key={inst.id}
                    instance={inst}
                    max={stats.maxPlaytime}
                    onClick={() => openInstance(inst.id)}
                  />
                ))}
              </div>
            )}
          </section>

          <div className="space-y-6">
            {/* Loader breakdown */}
            <section className="card-surface p-5">
              <h2 className="mb-4 text-sm font-semibold">By loader</h2>
              <div className="space-y-2.5">
                {stats.loaders.map(([loader, count]) => (
                  <div key={loader} className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">{loaderLabel(loader)}</span>
                    <span className="font-medium">{count}</span>
                  </div>
                ))}
              </div>
            </section>

            {/* Recently played */}
            <section className="card-surface p-5">
              <h2 className="mb-4 text-sm font-semibold">Recently played</h2>
              {stats.recent.length === 0 ? (
                <p className="text-sm text-muted-foreground">Nothing yet.</p>
              ) : (
                <div className="space-y-1">
                  {stats.recent.map((inst) => (
                    <button
                      key={inst.id}
                      onClick={() => openInstance(inst.id)}
                      className="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left transition-colors hover:bg-muted/60 btn-focus"
                    >
                      <InstanceIcon instance={inst} size={9} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm font-medium">{inst.name}</div>
                        <div className="truncate text-xs text-muted-foreground">
                          {timeAgo(inst.lastPlayed)}
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}

function Header() {
  const accounts = useStore((s) => s.accounts);
  const active = accounts.find((a) => a.active);
  return (
    <header className="px-8 pb-4 pt-6">
      <h1 className="text-2xl font-bold tracking-tight">
        {active ? `Welcome back, ${active.username}` : "Welcome to Beacon"}
      </h1>
      <p className="text-sm text-muted-foreground">Your launcher at a glance</p>
    </header>
  );
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="card-surface flex flex-col gap-3 p-5">
      <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-accent/15 text-accent">
        {icon}
      </div>
      <div>
        <div className="text-2xl font-bold tracking-tight">{value}</div>
        <div className="text-xs text-muted-foreground">{label}</div>
      </div>
    </div>
  );
}

function MostPlayedCard({
  instance,
  running,
  onOpen,
  onPlay,
}: {
  instance: Instance;
  running: boolean;
  onOpen: () => void;
  onPlay: () => void;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="mt-6 flex items-center gap-5 overflow-hidden card-surface p-5"
    >
      <InstanceIcon instance={instance} size={16} className="text-3xl" />
      <div className="min-w-0 flex-1">
        <div className="text-xs font-semibold uppercase tracking-wide text-accent">
          Most played
        </div>
        <button onClick={onOpen} className="truncate text-lg font-bold hover:underline">
          {instance.name}
        </button>
        <div className="text-sm text-muted-foreground">
          {formatDuration(instance.totalPlaySeconds)} · {instance.mcVersion}
          {instance.loader !== "vanilla" && ` · ${loaderLabel(instance.loader)}`}
        </div>
      </div>
      <button
        onClick={onPlay}
        disabled={running}
        className="inline-flex h-10 shrink-0 items-center gap-2 rounded-lg bg-accent px-4 text-sm font-semibold text-accent-foreground shadow-lg shadow-accent/25 transition hover:brightness-110 disabled:opacity-50 btn-focus"
      >
        <Play className="h-4 w-4 fill-current" />
        {running ? "Running" : "Play"}
      </button>
    </motion.div>
  );
}

function PlaytimeBar({
  instance,
  max,
  onClick,
}: {
  instance: Instance;
  max: number;
  onClick: () => void;
}) {
  const pct = max > 0 ? Math.max(4, (instance.totalPlaySeconds / max) * 100) : 0;
  return (
    <button onClick={onClick} className="group block w-full text-left btn-focus">
      <div className="mb-1 flex items-center justify-between text-sm">
        <span className="truncate font-medium group-hover:text-accent">{instance.name}</span>
        <span className="shrink-0 pl-3 text-xs text-muted-foreground">
          {formatDuration(instance.totalPlaySeconds)}
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-muted/60">
        <motion.div
          className="h-full rounded-full bg-accent"
          initial={{ width: 0 }}
          animate={{ width: `${pct}%` }}
          transition={{ type: "spring", stiffness: 120, damping: 20 }}
        />
      </div>
    </button>
  );
}

function InstanceIcon({
  instance,
  size,
  className,
}: {
  instance: Instance;
  size: number;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-accent/25 to-accent/5",
        className,
      )}
      style={{ height: `${size * 4}px`, width: `${size * 4}px` }}
    >
      {instance.icon ? (
        <span>{instance.icon}</span>
      ) : (
        <span className="font-bold uppercase text-accent">{instance.name.charAt(0)}</span>
      )}
    </div>
  );
}

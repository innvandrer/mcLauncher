import { useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { Activity, Boxes, Clock, Package, Play, Sparkles, Trophy, Users } from "lucide-react";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { InstanceIcon } from "@/components/InstanceIcon";
import { WrappedModal } from "@/components/WrappedModal";
import { cn, formatDuration, loaderLabel, timeAgo } from "@/lib/utils";
import type { Instance, Loader, Session } from "@/lib/types";

const SPLASHES = [
  "Now with 100% more blocks!",
  "Also try Redstone!",
  "Diamonds not included.",
  "Creeper? Aw man.",
  "Built different.",
  "git gud at mining.",
  "Sneak 100.",
  "Powered by good vibes.",
  "It's dangerous to go alone!",
  "No mobs were harmed.",
  "Respawn anchor charged.",
  "Loaded faster than a chunk.",
  "Punch a tree, get wood.",
  "Mine your own business.",
  "100% open source!",
  "Map-shaped.",
];

function HeroBanner({
  active,
  instance,
  running,
  totalSeconds,
  instanceCount,
  onPlay,
  onOpen,
  onWrapped,
}: {
  active?: { id: string; username: string } | null;
  instance: Instance | null;
  running: boolean;
  totalSeconds: number;
  instanceCount: number;
  onPlay: () => void;
  onOpen: () => void;
  onWrapped?: (() => void) | null;
}) {
  const splash = useMemo(() => SPLASHES[Math.floor(Math.random() * SPLASHES.length)], []);
  return (
    <div className="relative overflow-hidden rounded-2xl border border-white/5 bg-surface/30">
      <div
        className="absolute inset-0"
        style={{
          background:
            "radial-gradient(80% 140% at 0% 0%, hsl(var(--accent) / 0.30), transparent 60%), radial-gradient(70% 130% at 100% 0%, hsl(var(--accent) / 0.16), transparent 55%)",
        }}
      />
      {onWrapped && (
        <button
          onClick={onWrapped}
          className="absolute right-4 top-4 z-10 inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-foreground/80 backdrop-blur transition hover:bg-white/10 btn-focus"
          title="Your year in review"
        >
          <Sparkles className="h-3.5 w-3.5 text-accent" />
          Wrapped {new Date().getFullYear()}
        </button>
      )}
      <div className="relative flex flex-col gap-6 p-4 sm:flex-row sm:items-center sm:p-7">
        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold uppercase tracking-wider text-accent">Welcome back</p>
          <h1 className="mt-1 truncate text-2xl font-bold tracking-tight sm:text-3xl">
            {active ? active.username : "to EZMapa"}
          </h1>
          <p className="mt-1.5 text-sm text-muted-foreground">
            {formatDuration(totalSeconds)} played · {instanceCount}{" "}
            {instanceCount === 1 ? "instance" : "instances"}
            <span className="ml-0 mt-1 block select-none italic text-amber-400/90 sm:ml-2 sm:mt-0 sm:inline">
              {splash}
            </span>
          </p>
          {instance && (
            <div className="mt-5 flex flex-wrap items-center gap-3">
              <button
                onClick={onPlay}
                disabled={running}
                className="inline-flex h-11 max-w-full items-center gap-2 rounded-xl bg-accent px-5 text-sm font-semibold text-accent-foreground shadow-lg shadow-accent/30 transition hover:brightness-110 active:scale-[0.98] disabled:opacity-60 btn-focus"
              >
                <Play className="h-4 w-4 shrink-0 fill-current" />
                <span className="truncate">
                  {running ? `${instance.name} running` : `Continue — ${instance.name}`}
                </span>
              </button>
              <button
                onClick={onOpen}
                className="text-sm text-muted-foreground transition hover:text-foreground btn-focus"
              >
                {instance.mcVersion}
                {instance.loader !== "vanilla" ? ` · ${loaderLabel(instance.loader)}` : ""} →
              </button>
            </div>
          )}
        </div>
        {active && (
          <img
            src={`https://mc-heads.net/body/${encodeURIComponent(active.id)}/140`}
            alt=""
            className="hidden h-44 shrink-0 self-end [image-rendering:pixelated] sm:block"
          />
        )}
      </div>
    </div>
  );
}

export function HomePage() {
  const instances = useStore((s) => s.instances);
  const accounts = useStore((s) => s.accounts);
  const running = useStore((s) => s.running);
  const openInstance = useStore((s) => s.openInstance);
  const setView = useStore((s) => s.setView);
  const launch = useStore((s) => s.launch);
  const active = accounts.find((a) => a.active);

  const [sessions, setSessions] = useState<Session[]>([]);
  const [wrappedOpen, setWrappedOpen] = useState(false);
  useEffect(() => {
    api.listSessions().then(setSessions).catch(() => {});
  }, []);

  // Bucket play sessions into the last 14 local days for the activity chart.
  const activity = useMemo(() => {
    const DAYS = 14;
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const key = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
    const buckets = Array.from({ length: DAYS }, (_, i) => {
      const d = new Date(today);
      d.setDate(today.getDate() - (DAYS - 1 - i));
      return {
        key: key(d),
        tick: `${d.getDate()}`,
        full: d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" }),
        seconds: 0,
      };
    });
    const idx = new Map(buckets.map((b, i) => [b.key, i]));
    for (const s of sessions) {
      const d = new Date(s.started * 1000);
      d.setHours(0, 0, 0, 0);
      const i = idx.get(key(d));
      if (i != null) buckets[i].seconds += s.seconds;
    }
    const max = Math.max(1, ...buckets.map((b) => b.seconds));
    const total = buckets.reduce((a, b) => a + b.seconds, 0);
    return { buckets, max, total };
  }, [sessions]);

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
      <div className="app-page">
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
    <div className="app-page">
      <div className="app-scroll app-gutter pb-8 pt-6">
        <HeroBanner
          active={active}
          instance={stats.mostPlayed}
          running={stats.mostPlayed ? running.has(stats.mostPlayed.id) : false}
          totalSeconds={stats.totalSeconds}
          instanceCount={instances.length}
          onPlay={() => stats.mostPlayed && launch(stats.mostPlayed.id)}
          onOpen={() => stats.mostPlayed && openInstance(stats.mostPlayed.id)}
          onWrapped={sessions.length > 0 ? () => setWrappedOpen(true) : null}
        />
        <WrappedModal
          open={wrappedOpen}
          onClose={() => setWrappedOpen(false)}
          sessions={sessions}
        />
        {/* Stat cards */}
        <div className="mt-6 grid grid-cols-2 gap-4 lg:grid-cols-4">
          <StatCard
            icon={<Clock className="h-5 w-5" />}
            label="Total playtime"
            value={stats.totalSeconds > 0 ? formatDuration(stats.totalSeconds) : "—"}
          />
          <StatCard
            icon={<Boxes className="h-5 w-5" />}
            label="Instances"
            value={<CountUp value={instances.length} />}
          />
          <StatCard
            icon={<Package className="h-5 w-5" />}
            label="Mods installed"
            value={<CountUp value={stats.totalMods} />}
          />
          <StatCard
            icon={<Users className="h-5 w-5" />}
            label="Accounts"
            value={<CountUp value={accounts.length} />}
          />
        </div>

        {/* Play activity (last 14 days) */}
        {activity.total > 0 && (
          <div className="mt-6">
            <ActivityChart buckets={activity.buckets} max={activity.max} total={activity.total} />
          </div>
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
                      <InstanceIcon instance={inst} size="sm" />
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
  const splash = useMemo(() => SPLASHES[Math.floor(Math.random() * SPLASHES.length)], []);
  return (
    <header className="app-gutter pb-4 pt-6">
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <h1 className="text-2xl font-bold tracking-tight">
          {active ? `Welcome back, ${active.username}` : "Welcome to EZMapa"}
        </h1>
        <span className="-rotate-6 select-none text-sm font-semibold italic text-amber-400">
          {splash}
        </span>
      </div>
      <p className="text-sm text-muted-foreground">Your launcher at a glance</p>
    </header>
  );
}

function ActivityChart({
  buckets,
  max,
  total,
}: {
  buckets: { key: string; tick: string; full: string; seconds: number }[];
  max: number;
  total: number;
}) {
  return (
    <section className="card-surface min-w-0 overflow-hidden p-5">
      <h2 className="mb-4 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm font-semibold">
        <Activity className="h-4 w-4 shrink-0 text-accent" />
        Play activity
        <span className="text-xs font-normal text-muted-foreground sm:ml-auto">
          {formatDuration(total)} · last 14 days
        </span>
      </h2>
      <div className="flex h-28 min-w-0 items-end gap-1 sm:gap-1.5">
        {buckets.map((b) => (
          <div key={b.key} className="flex h-full min-w-0 flex-1 items-end" title={`${b.full}: ${b.seconds > 0 ? formatDuration(b.seconds) : "no play"}`}>
            <motion.div
              initial={{ height: 0 }}
              animate={{ height: `${(b.seconds / max) * 100}%` }}
              transition={{ type: "spring", stiffness: 120, damping: 20 }}
              className="w-full rounded-t bg-accent/80 transition-colors hover:bg-accent"
              style={{ minHeight: b.seconds > 0 ? 3 : 0 }}
            />
          </div>
        ))}
      </div>
      <div className="mt-1 flex min-w-0 gap-1 sm:gap-1.5">
        {buckets.map((b, i) => (
          <span key={b.key} className="min-w-0 flex-1 truncate text-center text-[10px] text-muted-foreground">
            {i % 2 === 0 ? b.tick : ""}
          </span>
        ))}
      </div>
    </section>
  );
}

/** Animates a number from 0 to `value` once on mount / when it changes. */
function CountUp({ value }: { value: number }) {
  const [n, setN] = useState(0);
  useEffect(() => {
    if (value <= 0) {
      setN(0);
      return;
    }
    let raf = 0;
    const start = performance.now();
    const dur = 650;
    const tick = (t: number) => {
      const p = Math.min(1, (t - start) / dur);
      const eased = 1 - Math.pow(1 - p, 3);
      setN(Math.round(value * eased));
      if (p < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [value]);
  return <>{n}</>;
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: React.ReactNode;
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


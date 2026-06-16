import { useState } from "react";
import { motion } from "framer-motion";
import {
  Copy,
  FolderOpen,
  Loader2,
  MoreVertical,
  Play,
  Star,
  Trash2,
  X,
} from "lucide-react";
import type { Instance } from "@/lib/types";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { ACCENTS, cn, formatPlaytime, loaderLabel, timeAgo } from "@/lib/utils";
import { InstanceIcon } from "./InstanceIcon";

const LOADER_THEMES: Record<string, string> = {
  vanilla: "from-emerald-600/20 to-emerald-900/5 border-emerald-500/20",
  fabric: "from-blue-600/20 to-yellow-500/5 border-blue-500/20",
  forge: "from-orange-600/20 to-red-600/5 border-orange-500/20",
  neoforge: "from-cyan-600/20 to-blue-600/5 border-cyan-500/20",
  quilt: "from-purple-600/20 to-pink-600/5 border-purple-500/20",
};

export function InstanceCard({ instance, onOpen }: { instance: Instance; onOpen: () => void }) {
  const running = useStore((s) => s.running.has(instance.id));
  const installTask = useStore((s) => s.tasks[`modpack:${instance.id}`]);
  const installing = !!installTask && !installTask.done;
  const installPct =
    installTask && installTask.total > 0
      ? Math.round((installTask.current / installTask.total) * 100)
      : 0;
  const launch = useStore((s) => s.launch);
  const stop = useStore((s) => s.stop);
  const duplicate = useStore((s) => s.duplicateInstance);
  const remove = useStore((s) => s.deleteInstance);
  const updateInstance = useStore((s) => s.updateInstance);
  const toast = useStore((s) => s.toast);
  const [menuOpen, setMenuOpen] = useState(false);

  const toggleFavorite = (e: React.MouseEvent) => {
    e.stopPropagation();
    updateInstance({ ...instance, favorite: !instance.favorite });
  };

  const isModded = instance.loader !== "vanilla";
  const isATM10 = instance.name.toLowerCase().includes("all the mods 10");

  // A user-chosen accent overrides the loader gradient with a colour tint.
  const accentHsl = instance.accent ? ACCENTS[instance.accent] : null;
  const accentStyle = accentHsl
    ? {
        borderColor: `hsl(${accentHsl} / 0.4)`,
        background: `linear-gradient(to bottom right, hsl(${accentHsl} / 0.18), hsl(${accentHsl} / 0.02))`,
      }
    : undefined;

  const theme = isATM10
    ? "from-yellow-600/30 to-zinc-900/80 border-yellow-500/40"
    : (LOADER_THEMES[instance.loader] || LOADER_THEMES.vanilla);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      whileHover={{ y: -3 }}
      transition={{ type: "spring", stiffness: 300, damping: 26 }}
      onClick={onOpen}
      style={accentStyle}
      className={cn(
        "group relative flex cursor-pointer flex-col overflow-hidden border bg-gradient-to-br p-4 rounded-xl shadow-sm transition-all hover:shadow-xl hover:shadow-black/20",
        accentHsl ? "border" : theme,
      )}
    >
      {/* Icon + menu */}
      <div className="mb-3 flex items-start justify-between">
        <InstanceIcon instance={instance} size="card" />

        <div className="flex items-center gap-0.5">
        <button
          onClick={toggleFavorite}
          title={instance.favorite ? "Unpin" : "Pin to top"}
          className={cn(
            "rounded-md p-1.5 transition btn-focus",
            instance.favorite
              ? "text-amber-400 opacity-100"
              : "text-muted-foreground opacity-0 hover:bg-muted hover:text-foreground group-hover:opacity-100",
          )}
        >
          <Star className={cn("h-4 w-4", instance.favorite && "fill-current")} />
        </button>
        <div className="relative">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setMenuOpen((v) => !v);
            }}
            onBlur={() => setTimeout(() => setMenuOpen(false), 150)}
            className="rounded-md p-1.5 text-muted-foreground opacity-0 transition hover:bg-muted hover:text-foreground group-hover:opacity-100 btn-focus"
          >
            <MoreVertical className="h-4 w-4" />
          </button>
          {menuOpen && (
            <div className="absolute right-0 top-9 z-20 w-44 overflow-hidden rounded-lg border bg-card py-1 shadow-xl animate-fade-in">
              <MenuItem
                icon={<FolderOpen className="h-4 w-4" />}
                label="Open folder"
                onClick={() => api.openInstanceFolder(instance.id)}
              />
              <MenuItem
                icon={<Copy className="h-4 w-4" />}
                label="Duplicate"
                onClick={() => duplicate(instance.id)}
              />
              <MenuItem
                icon={<Trash2 className="h-4 w-4" />}
                label="Delete"
                danger
                onClick={() => {
                  if (running) {
                    toast("error", "Stop the instance before deleting it.");
                    return;
                  }
                  if (confirm(`Delete “${instance.name}”? This cannot be undone.`))
                    remove(instance.id);
                }}
              />
            </div>
          )}
        </div>
        </div>
      </div>

      {/* Title + meta */}
      <h3 className="truncate text-[15px] font-semibold">{instance.name}</h3>
      <div className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
        <span>{instance.mcVersion}</span>
        {isModded && (
          <>
            <span className="opacity-40">•</span>
            <span className="font-medium text-accent">{loaderLabel(instance.loader)}</span>
          </>
        )}
      </div>
      <p className="mt-0.5 text-xs text-muted-foreground/80">
        {instance.lastPlayed
          ? `Played ${timeAgo(instance.lastPlayed)}`
          : formatPlaytime(instance.totalPlaySeconds)}
        {isModded && instance.modCount != null && instance.modCount > 0 && (
          <span className="ml-1 opacity-70">· {instance.modCount} mod{instance.modCount !== 1 ? "s" : ""}</span>
        )}
      </p>

      {/* Play / Stop / Installing */}
      {installing ? (
        <div className="mt-4">
          <div className="flex items-center gap-2">
            <div
              className="inline-flex h-9 flex-1 items-center justify-center gap-2 rounded-lg bg-accent/15 text-sm font-semibold text-accent"
              title={installTask?.currentFile ?? "Installing"}
            >
              <Loader2 className="h-4 w-4 animate-spin" />
              Installing… {installPct}%
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                api.cancelTask(`modpack:${instance.id}`);
              }}
              title="Cancel install"
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground transition hover:bg-destructive/15 hover:text-destructive btn-focus"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
          <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-muted">
            <motion.div
              className="h-full rounded-full bg-accent"
              animate={{ width: `${installPct}%` }}
              transition={{ ease: "easeOut", duration: 0.2 }}
            />
          </div>
        </div>
      ) : (
        <button
          onClick={(e) => {
            e.stopPropagation();
            running ? stop(instance.id) : launch(instance.id);
          }}
          className={cn(
            "mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-lg text-sm font-semibold transition-all active:scale-[0.98] btn-focus",
            running
              ? "bg-destructive/15 text-destructive hover:bg-destructive/25"
              : "bg-accent text-accent-foreground hover:brightness-110 shadow-md shadow-accent/20",
          )}
        >
          {running ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-destructive opacity-75" />
                <span className="relative inline-flex h-2 w-2 rounded-full bg-destructive" />
              </span>
              Stop
            </>
          ) : (
            <>
              <Play className="h-4 w-4 fill-current" />
              Play
            </>
          )}
        </button>
      )}
    </motion.div>
  );
}

function MenuItem({
  icon,
  label,
  onClick,
  danger,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className={cn(
        "flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm transition-colors",
        danger
          ? "text-destructive hover:bg-destructive/10"
          : "text-foreground hover:bg-muted",
      )}
    >
      {icon}
      {label}
    </button>
  );
}

import { useState } from "react";
import { motion } from "framer-motion";
import {
  Copy,
  FolderOpen,
  MoreVertical,
  Play,
  Square,
  Trash2,
} from "lucide-react";
import type { Instance } from "@/lib/types";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { cn, formatPlaytime, loaderLabel, timeAgo } from "@/lib/utils";

export function InstanceCard({ instance, onOpen }: { instance: Instance; onOpen: () => void }) {
  const running = useStore((s) => s.running.has(instance.id));
  const launch = useStore((s) => s.launch);
  const stop = useStore((s) => s.stop);
  const duplicate = useStore((s) => s.duplicateInstance);
  const remove = useStore((s) => s.deleteInstance);
  const toast = useStore((s) => s.toast);
  const [menuOpen, setMenuOpen] = useState(false);

  const isModded = instance.loader !== "vanilla";

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      whileHover={{ y: -3 }}
      transition={{ type: "spring", stiffness: 300, damping: 26 }}
      onClick={onOpen}
      className="group relative flex cursor-pointer flex-col overflow-hidden card-surface p-4 transition-shadow hover:shadow-xl hover:shadow-black/20"
    >
      {/* Icon + menu */}
      <div className="mb-3 flex items-start justify-between">
        <div className="flex h-14 w-14 items-center justify-center rounded-xl bg-gradient-to-br from-accent/25 to-accent/5 text-2xl">
          {instance.icon ? (
            <span>{instance.icon}</span>
          ) : (
            <span className="font-bold uppercase text-accent">{instance.name.charAt(0)}</span>
          )}
        </div>

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
      </p>

      {/* Play / Stop */}
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

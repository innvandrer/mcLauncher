import { AnimatePresence, motion } from "framer-motion";
import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { cn, formatBytes, formatEta, formatSpeed } from "@/lib/utils";

export function Toaster() {
  const toasts = useStore((s) => s.toasts);
  const tasks = useStore((s) => s.tasks);
  const taskStarted = useStore((s) => s.taskStarted);
  const dismiss = useStore((s) => s.dismissToast);
  const taskList = Object.values(tasks);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
      <AnimatePresence>
        {taskList.map((t) => {
          const pct = t.total > 0 ? Math.round((t.current / t.total) * 100) : 0;
          const speed = formatSpeed(t.speed);
          const eta = t.done
            ? ""
            : formatEta(taskStarted[t.id] ?? Date.now(), t.current, t.total);
          // Build the meta line: "142 / 318 · 47.3 MB · 12.1 MB/s · 18s left".
          const meta = [
            t.total > 0 ? `${t.current} / ${t.total}` : null,
            t.bytesDone ? formatBytes(t.bytesDone) : null,
            speed || null,
            eta || null,
          ].filter(Boolean);
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, x: 40 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 40 }}
              className="pointer-events-auto overflow-hidden card-surface p-3 shadow-lg"
            >
              <div className="flex items-center justify-between gap-2 text-sm">
                <span className="truncate font-medium">{t.label}</span>
                <div className="flex shrink-0 items-center gap-1.5">
                  <span className="text-xs text-muted-foreground">
                    {t.done ? "Done" : `${pct}%`}
                  </span>
                  {!t.done && t.id.startsWith("modpack:") && (
                    <button
                      onClick={() => api.cancelTask(t.id)}
                      title="Cancel"
                      className="text-muted-foreground transition hover:text-destructive"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  )}
                </div>
              </div>
              <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <motion.div
                  className={cn(
                    "h-full rounded-full",
                    t.done ? "bg-success" : "bg-accent",
                  )}
                  animate={{ width: `${pct}%` }}
                  transition={{ ease: "easeOut", duration: 0.2 }}
                />
              </div>
              {meta.length > 0 && (
                <div className="mt-1 flex items-center gap-1.5 text-[11px] text-muted-foreground">
                  {meta.map((m, i) => (
                    <span key={i} className="flex items-center gap-1.5">
                      {i > 0 && <span className="opacity-40">·</span>}
                      <span>{m}</span>
                    </span>
                  ))}
                </div>
              )}
              {t.currentFile && !t.done && (
                <div className="mt-0.5 truncate text-[11px] text-muted-foreground/70">
                  {t.currentFile}
                </div>
              )}
            </motion.div>
          );
        })}

        {toasts.map((t) => (
          <motion.div
            key={t.id}
            layout
            initial={{ opacity: 0, x: 40 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 40, scale: 0.95 }}
            className={cn(
              "pointer-events-auto flex items-start gap-2.5 card-surface p-3 shadow-lg",
              t.type === "error" && "border-destructive/40",
            )}
          >
            <div className="mt-0.5">
              {t.type === "success" && (
                <CheckCircle2 className="h-4 w-4 text-success" />
              )}
              {t.type === "error" && (
                <AlertCircle className="h-4 w-4 text-destructive" />
              )}
              {t.type === "info" && (
                <Info className="h-4 w-4 text-muted-foreground" />
              )}
            </div>
            <p className="flex-1 text-sm leading-snug">{t.message}</p>
            <button
              onClick={() => dismiss(t.id)}
              className="text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

import { AnimatePresence, motion } from "framer-motion";
import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useStore } from "@/store/useStore";
import { cn } from "@/lib/utils";

export function Toaster() {
  const toasts = useStore((s) => s.toasts);
  const tasks = useStore((s) => s.tasks);
  const dismiss = useStore((s) => s.dismissToast);
  const taskList = Object.values(tasks);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
      <AnimatePresence>
        {taskList.map((t) => {
          const pct = t.total > 0 ? Math.round((t.current / t.total) * 100) : 0;
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, x: 40 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 40 }}
              className="pointer-events-auto overflow-hidden card-surface p-3 shadow-lg"
            >
              <div className="flex items-center justify-between text-sm">
                <span className="font-medium">{t.label}</span>
                <span className="text-xs text-muted-foreground">{pct}%</span>
              </div>
              <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <motion.div
                  className="h-full rounded-full bg-accent"
                  animate={{ width: `${pct}%` }}
                  transition={{ ease: "easeOut", duration: 0.2 }}
                />
              </div>
              {t.total > 0 && (
                <div className="mt-1 text-[11px] text-muted-foreground">
                  {t.current} / {t.total} files
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
              {t.type === "success" && <CheckCircle2 className="h-4 w-4 text-success" />}
              {t.type === "error" && <AlertCircle className="h-4 w-4 text-destructive" />}
              {t.type === "info" && <Info className="h-4 w-4 text-muted-foreground" />}
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

import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
  Bell,
  CheckCircle2,
  CircleAlert,
  Eraser,
  History,
  X,
} from "lucide-react";
import {
  clearActivities,
  useActivities,
  type ActivityKind,
} from "@/lib/activity";
import { useStore } from "@/store/useStore";
import { cn } from "@/lib/utils";

const FILTERS: { id: "all" | ActivityKind; label: string }[] = [
  { id: "all", label: "All" },
  { id: "content", label: "Updates" },
  { id: "backup", label: "Recovery" },
  { id: "crash", label: "Crashes" },
];

export function ActivityCenter() {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["id"]>("all");
  const activity = useActivities();
  const taskMap = useStore((s) => s.tasks);
  const tasks = useMemo(() => Object.values(taskMap), [taskMap]);
  const instances = useStore((s) => s.instances);
  const shown = useMemo(
    () => activity.filter((item) => filter === "all" || item.kind === filter),
    [activity, filter],
  );

  useEffect(() => {
    if (!open) return;

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [open]);

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="relative flex h-10 w-11 items-center justify-center text-muted-foreground transition hover:bg-muted hover:text-foreground btn-focus"
        aria-label="Activity center"
        aria-expanded={open}
      >
        <Bell className="h-4 w-4" />
        {(tasks.some((task) => !task.done) || activity.length > 0) && (
          <span className="absolute right-2 top-2 h-1.5 w-1.5 rounded-full bg-accent" />
        )}
      </button>
      {open &&
        createPortal(
          <div
            className="fixed inset-0 z-[80]"
            role="dialog"
            aria-label="Activity center"
          >
            <button
              className="absolute inset-0 bg-black/35"
              onClick={() => setOpen(false)}
              aria-label="Close activity center"
            />
            <aside className="absolute bottom-3 right-3 top-12 flex w-[min(420px,calc(100vw-24px))] flex-col overflow-hidden rounded-2xl border bg-background/95 shadow-2xl glass">
              <header className="flex items-center gap-3 border-b px-4 py-3">
                <History className="h-4 w-4 text-accent" />
                <div className="min-w-0 flex-1">
                  <h2 className="font-semibold">Activity center</h2>
                  <p className="text-xs text-muted-foreground">
                    Tasks, updates, recovery, and crash history
                  </p>
                </div>
                <button
                  onClick={() => setOpen(false)}
                  className="rounded-lg p-2 hover:bg-muted btn-focus"
                  aria-label="Close"
                >
                  <X className="h-4 w-4" />
                </button>
              </header>

              {tasks.some((task) => !task.done) && (
                <section className="space-y-2 border-b p-4">
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    In progress
                  </p>
                  {tasks
                    .filter((task) => !task.done)
                    .map((task) => {
                      const percent = task.total
                        ? Math.round((task.current / task.total) * 100)
                        : 0;
                      return (
                        <div
                          key={task.id}
                          className="rounded-xl border bg-card/70 p-3"
                        >
                          <div className="flex justify-between gap-3 text-sm">
                            <span className="truncate font-medium">
                              {task.label}
                            </span>
                            <span>{percent}%</span>
                          </div>
                          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
                            <div
                              className="h-full rounded-full bg-accent transition-all"
                              style={{ width: `${percent}%` }}
                            />
                          </div>
                          <p className="mt-1.5 truncate text-xs text-muted-foreground">
                            {task.stage}
                            {task.currentFile ? ` · ${task.currentFile}` : ""}
                          </p>
                        </div>
                      );
                    })}
                </section>
              )}

              <div className="flex items-center gap-1 overflow-x-auto border-b px-3 py-2">
                {FILTERS.map((item) => (
                  <button
                    key={item.id}
                    onClick={() => setFilter(item.id)}
                    className={cn(
                      "rounded-lg px-2.5 py-1.5 text-xs font-medium",
                      filter === item.id
                        ? "bg-accent/15 text-accent"
                        : "text-muted-foreground hover:bg-muted",
                    )}
                  >
                    {item.label}
                  </button>
                ))}
                <button
                  onClick={clearActivities}
                  className="ml-auto rounded-lg p-2 text-muted-foreground hover:bg-muted hover:text-foreground"
                  title="Clear history"
                >
                  <Eraser className="h-3.5 w-3.5" />
                </button>
              </div>

              <div className="min-h-0 flex-1 overflow-y-auto p-3">
                {shown.length === 0 ? (
                  <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-muted-foreground">
                    <CheckCircle2 className="h-8 w-8 opacity-50" />
                    <div>
                      <p className="text-sm font-medium text-foreground">
                        All quiet
                      </p>
                      <p className="text-xs">
                        New launcher activity will appear here.
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="space-y-1">
                    {shown.map((item) => {
                      const instance = instances.find(
                        (candidate) => candidate.id === item.instanceId,
                      );
                      return (
                        <div
                          key={item.id}
                          className="flex gap-3 rounded-xl p-3 hover:bg-muted/50"
                        >
                          <div
                            className={cn(
                              "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg",
                              item.kind === "crash"
                                ? "bg-destructive/15 text-destructive"
                                : "bg-accent/12 text-accent",
                            )}
                          >
                            {item.kind === "crash" ? (
                              <CircleAlert className="h-3.5 w-3.5" />
                            ) : (
                              <CheckCircle2 className="h-3.5 w-3.5" />
                            )}
                          </div>
                          <div className="min-w-0 flex-1">
                            <p className="text-sm">{item.message}</p>
                            {(instance || item.detail) && (
                              <p className="mt-0.5 truncate text-xs text-muted-foreground">
                                {instance?.name}
                                {instance && item.detail ? " · " : ""}
                                {item.detail}
                              </p>
                            )}
                            <p className="mt-1 text-[11px] text-muted-foreground">
                              {new Date(item.created).toLocaleString()}
                            </p>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </aside>
          </div>,
          document.body,
        )}
    </>
  );
}

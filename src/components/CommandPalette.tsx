import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Boxes,
  Home,
  Package,
  Play,
  Search,
  Settings,
  Square,
  User,
} from "lucide-react";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { cn, isImageIcon } from "@/lib/utils";

interface Command {
  id: string;
  label: string;
  hint?: string;
  icon: React.ReactNode;
  keywords: string;
  run: () => void;
}

/** Global ⌘K / Ctrl-K command palette: fuzzy-jump to instances, views, and
 *  actions without leaving the keyboard. */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const instances = useStore((s) => s.instances);
  const running = useStore((s) => s.running);
  const setView = useStore((s) => s.setView);
  const openInstance = useStore((s) => s.openInstance);
  const launch = useStore((s) => s.launch);
  const stop = useStore((s) => s.stop);

  // Toggle on ⌘K / Ctrl-K from anywhere.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((v) => !v);
      } else if (e.key === "Escape") {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [open]);

  const close = () => setOpen(false);

  const commands = useMemo<Command[]>(() => {
    const views: Command[] = [
      { id: "v:home", label: "Go to Home", icon: <Home className="h-4 w-4" />, keywords: "home dashboard", run: () => setView("home") },
      { id: "v:instances", label: "Go to Instances", icon: <Boxes className="h-4 w-4" />, keywords: "instances", run: () => setView("instances") },
      { id: "v:modpacks", label: "Browse Modpacks", icon: <Package className="h-4 w-4" />, keywords: "modpacks browse install", run: () => setView("modpacks") },
      { id: "v:accounts", label: "Go to Accounts", icon: <User className="h-4 w-4" />, keywords: "accounts login microsoft", run: () => setView("accounts") },
      { id: "v:settings", label: "Open Settings", icon: <Settings className="h-4 w-4" />, keywords: "settings ram memory java theme", run: () => setView("settings") },
    ];

    const perInstance: Command[] = instances.flatMap((inst) => {
      const isRunning = running.has(inst.id);
      const icon = isImageIcon(inst.icon) ? (
        <img src={inst.icon!} alt="" className="h-4 w-4 rounded object-cover" />
      ) : (
        <span className="text-sm leading-none">{inst.icon ?? "📦"}</span>
      );
      return [
        {
          id: `open:${inst.id}`,
          label: `Open ${inst.name}`,
          hint: `${inst.mcVersion} · ${inst.loader}`,
          icon,
          keywords: `${inst.name} ${inst.mcVersion} ${inst.loader} open`,
          run: () => openInstance(inst.id),
        },
        {
          id: `play:${inst.id}`,
          label: isRunning ? `Stop ${inst.name}` : `Play ${inst.name}`,
          icon: isRunning ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />,
          keywords: `${inst.name} ${isRunning ? "stop" : "play launch run"}`,
          run: () => (isRunning ? stop(inst.id) : launch(inst.id)),
        },
      ];
    });

    return [...views, ...perInstance];
  }, [instances, running, setView, openInstance, launch, stop]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands.slice(0, 8);
    return commands
      .filter((c) => `${c.label} ${c.keywords}`.toLowerCase().includes(q))
      .slice(0, 12);
  }, [commands, query]);

  useEffect(() => {
    if (active >= filtered.length) setActive(0);
  }, [filtered.length, active]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cmd = filtered[active];
      if (cmd) {
        cmd.run();
        close();
      }
    }
  };

  // Keep the active row scrolled into view.
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${active}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [active]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[80] flex items-start justify-center bg-black/50 pt-[12vh] backdrop-blur-sm"
          onClick={close}
        >
          <motion.div
            initial={{ opacity: 0, y: -12, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.98 }}
            transition={{ duration: 0.15 }}
            onClick={(e) => e.stopPropagation()}
            className="w-full max-w-xl overflow-hidden rounded-xl border bg-card shadow-2xl"
          >
            <div className="flex items-center gap-2 border-b px-4">
              <Search className="h-4 w-4 shrink-0 text-muted-foreground" />
              <input
                ref={inputRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={onKeyDown}
                placeholder="Search instances, actions…"
                className="h-12 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
              />
              <kbd className="rounded border px-1.5 py-0.5 text-[10px] text-muted-foreground">ESC</kbd>
            </div>
            <div ref={listRef} className="max-h-80 overflow-y-auto p-2">
              {filtered.length === 0 ? (
                <p className="px-3 py-6 text-center text-sm text-muted-foreground">
                  No matches for “{query}”.
                </p>
              ) : (
                filtered.map((c, i) => (
                  <button
                    key={c.id}
                    data-idx={i}
                    onClick={() => {
                      c.run();
                      close();
                    }}
                    onMouseMove={() => setActive(i)}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors",
                      i === active ? "bg-accent/15 text-foreground" : "text-foreground/90",
                    )}
                  >
                    <span className="flex h-5 w-5 items-center justify-center text-muted-foreground">
                      {c.icon}
                    </span>
                    <span className="flex-1 truncate">{c.label}</span>
                    {c.hint && (
                      <span className="shrink-0 text-xs text-muted-foreground">{c.hint}</span>
                    )}
                  </button>
                ))
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

import { Boxes, LayoutGrid, Settings as SettingsIcon, UserCircle2, Users } from "lucide-react";
import { motion } from "framer-motion";
import { useStore, type View } from "@/store/useStore";
import { cn } from "@/lib/utils";

const items: { id: View; label: string; icon: typeof LayoutGrid }[] = [
  { id: "instances", label: "Instances", icon: LayoutGrid },
  { id: "modpacks", label: "Modpacks", icon: Boxes },
  { id: "accounts", label: "Accounts", icon: Users },
  { id: "settings", label: "Settings", icon: SettingsIcon },
];

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const accounts = useStore((s) => s.accounts);
  const active = accounts.find((a) => a.active);

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-border/60 bg-surface/30 p-3">
      <nav className="flex flex-col gap-1">
        {items.map((item) => {
          const Icon = item.icon;
          const isActive = view === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setView(item.id)}
              className={cn(
                "relative flex items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors btn-focus",
                isActive ? "text-foreground" : "text-muted-foreground hover:text-foreground hover:bg-muted/50",
              )}
            >
              {isActive && (
                <motion.div
                  layoutId="nav-active"
                  className="absolute inset-0 rounded-lg bg-muted"
                  transition={{ type: "spring", stiffness: 380, damping: 32 }}
                />
              )}
              <Icon className="relative z-10 h-[18px] w-[18px]" />
              <span className="relative z-10">{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto">
        <button
          onClick={() => setView("accounts")}
          className="flex w-full items-center gap-3 rounded-lg border border-border/60 bg-card/60 p-2.5 text-left transition-colors hover:bg-muted/50 btn-focus"
        >
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent/15 text-accent">
            {active ? (
              <span className="text-sm font-bold uppercase">{active.username.charAt(0)}</span>
            ) : (
              <UserCircle2 className="h-5 w-5" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium">
              {active ? active.username : "No account"}
            </div>
            <div className="truncate text-xs text-muted-foreground">
              {active ? (active.kind === "offline" ? "Offline" : "Microsoft") : "Click to add"}
            </div>
          </div>
        </button>
      </div>
    </aside>
  );
}

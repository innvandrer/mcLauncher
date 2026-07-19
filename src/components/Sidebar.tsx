import {
  Boxes,
  Home,
  LayoutGrid,
  Settings as SettingsIcon,
  Users,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { motion } from "framer-motion";
import { useStore, type View } from "@/store/useStore";
import { cn } from "@/lib/utils";
import { PlayerAvatar } from "@/components/PlayerAvatar";
import { t, type StringKey } from "@/lib/strings";

const items: { id: View; label: StringKey; icon: typeof LayoutGrid }[] = [
  { id: "home", label: "nav.home", icon: Home },
  { id: "instances", label: "nav.instances", icon: LayoutGrid },
  { id: "modpacks", label: "nav.modpacks", icon: Boxes },
  { id: "accounts", label: "nav.accounts", icon: Users },
  { id: "settings", label: "nav.settings", icon: SettingsIcon },
];

export function Sidebar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const accounts = useStore((s) => s.accounts);
  const active = accounts.find((a) => a.active);
  const anyRunning = useStore((s) => s.running.size > 0);
  const settings = useStore((s) => s.settings);
  const saveSettings = useStore((s) => s.saveSettings);
  const collapsed = settings?.sidebarCollapsed ?? false;

  return (
    <aside
      className={cn(
        "flex shrink-0 flex-col border-r border-border/60 bg-surface/30 p-3 transition-[width]",
        collapsed ? "w-[68px]" : "w-56",
      )}
    >
      <button
        onClick={() =>
          settings &&
          saveSettings({ ...settings, sidebarCollapsed: !collapsed })
        }
        className="mb-2 flex items-center justify-center rounded-lg p-2 text-muted-foreground hover:bg-muted hover:text-foreground"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        {collapsed ? (
          <PanelLeftOpen className="h-4 w-4" />
        ) : (
          <PanelLeftClose className="h-4 w-4" />
        )}
      </button>
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
                isActive
                  ? "text-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-muted/50",
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
              {!collapsed && (
                <span className="relative z-10">{t(item.label)}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="mt-auto">
        <button
          onClick={() => setView("accounts")}
          className="flex w-full items-center gap-3 rounded-xl border border-border/60 bg-gradient-to-br from-card/80 to-card/40 p-2.5 text-left transition-colors hover:from-muted/60 hover:to-muted/30 btn-focus"
        >
          <div className="relative shrink-0">
            <PlayerAvatar
              account={active}
              size={38}
              className="ring-2 ring-accent/30 ring-offset-2 ring-offset-background"
            />
            {anyRunning && (
              <span className="absolute -bottom-0.5 -right-0.5 flex h-3 w-3">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-70" />
                <span className="relative inline-flex h-3 w-3 rounded-full border-2 border-background bg-emerald-400" />
              </span>
            )}
          </div>
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium">
                {active ? active.username : t("account.none")}
              </div>
              <div className="truncate text-xs text-muted-foreground">
                {active
                  ? anyRunning
                    ? t("account.playing")
                    : active.kind === "offline"
                      ? t("account.offline")
                      : t("account.microsoft")
                  : t("account.add")}
              </div>
            </div>
          )}
        </button>
      </div>
    </aside>
  );
}

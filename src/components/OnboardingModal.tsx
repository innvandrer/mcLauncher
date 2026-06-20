import { useEffect, useState } from "react";
import { Boxes, Package, Server, Sparkles, Users } from "lucide-react";
import { Button, Modal } from "@/components/ui";
import { useStore } from "@/store/useStore";

const SEEN_KEY = "ezmapa:onboarded";
const LEGACY_SEEN_KEY = "beacon:onboarded";

const FEATURES = [
  {
    icon: Boxes,
    title: "Isolated instances",
    desc: "Every pack gets its own mods, worlds and settings — nothing collides.",
  },
  {
    icon: Package,
    title: "Mods & modpacks",
    desc: "Browse Modrinth and CurseForge and install with a single click.",
  },
  {
    icon: Server,
    title: "Live server browser",
    desc: "See MOTD, player counts and ping for your servers — and jump straight in.",
  },
  {
    icon: Users,
    title: "Your account",
    desc: "Sign in with Microsoft, or play offline. Switch any time.",
  },
];

/** A one-time welcome shown on the very first launch. */
export function OnboardingModal() {
  const ready = useStore((s) => s.ready);
  const setView = useStore((s) => s.setView);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (ready && !localStorage.getItem(SEEN_KEY) && !localStorage.getItem(LEGACY_SEEN_KEY))
      setOpen(true);
  }, [ready]);

  const finish = (go?: "instances" | "accounts") => {
    localStorage.setItem(SEEN_KEY, "1");
    setOpen(false);
    if (go) setView(go);
  };

  return (
    <Modal
      open={open}
      onClose={() => finish()}
      title={
        <span className="flex items-center gap-2">
          <Sparkles className="h-4 w-4 text-accent" /> Welcome to EZMapa
        </span>
      }
      footer={
        <>
          <Button variant="ghost" onClick={() => finish()}>
            Skip
          </Button>
          <Button variant="secondary" onClick={() => finish("accounts")}>
            Add account
          </Button>
          <Button variant="primary" onClick={() => finish("instances")}>
            Create an instance
          </Button>
        </>
      }
    >
      <p className="mb-4 text-sm text-muted-foreground">
        A modern, open-source Minecraft launcher. Here's what you can do:
      </p>
      <div className="space-y-3">
        {FEATURES.map((f) => {
          const Icon = f.icon;
          return (
            <div key={f.title} className="flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent/15 text-accent">
                <Icon className="h-4 w-4" />
              </div>
              <div>
                <p className="text-sm font-medium">{f.title}</p>
                <p className="text-xs text-muted-foreground">{f.desc}</p>
              </div>
            </div>
          );
        })}
      </div>
      <p className="mt-4 text-xs text-muted-foreground/70">
        Tip: press <kbd className="rounded border px-1 py-0.5">Ctrl</kbd>
        <kbd className="ml-0.5 rounded border px-1 py-0.5">K</kbd> any time for the command
        palette, or <kbd className="rounded border px-1 py-0.5">?</kbd> for keyboard shortcuts.
      </p>
    </Modal>
  );
}

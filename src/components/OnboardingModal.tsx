import { useEffect, useMemo, useState } from "react";
import {
  Check,
  Circle,
  Keyboard,
  PackagePlus,
  Sparkles,
  UserRound,
} from "lucide-react";
import { Button, Modal } from "@/components/ui";
import { useStore } from "@/store/useStore";

const SEEN_KEY = "ezmapa:onboarded:v2";

export function OnboardingModal() {
  const ready = useStore((s) => s.ready);
  const accounts = useStore((s) => s.accounts);
  const instances = useStore((s) => s.instances);
  const setView = useStore((s) => s.setView);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (ready && !localStorage.getItem(SEEN_KEY)) setOpen(true);
  }, [ready]);
  const steps = useMemo(
    () => [
      {
        title: "Add an account",
        detail: "Microsoft or offline — you can switch later.",
        done: accounts.length > 0,
        icon: UserRound,
        action: () => {
          setOpen(false);
          setView("accounts");
        },
      },
      {
        title: "Create or import an instance",
        detail: "Each setup keeps its own mods, worlds, and settings.",
        done: instances.length > 0,
        icon: PackagePlus,
        action: () => {
          setOpen(false);
          setView("instances");
        },
      },
      {
        title: "Learn the fast lane",
        detail:
          "Press Ctrl+K to launch, navigate, and search without reaching for the mouse.",
        done: true,
        icon: Keyboard,
        action: () => {},
      },
    ],
    [accounts.length, instances.length, setView],
  );
  const complete = steps.filter((step) => step.done).length;
  const finish = () => {
    localStorage.setItem(SEEN_KEY, "1");
    setOpen(false);
  };
  return (
    <Modal
      open={open}
      onClose={finish}
      title={
        <span className="flex items-center gap-2">
          <Sparkles className="h-4 w-4 text-accent" /> Welcome to EZMapa
        </span>
      }
      footer={
        <>
          <Button variant="ghost" onClick={finish}>
            Dismiss checklist
          </Button>
          <Button
            variant="primary"
            onClick={() =>
              steps.find((step) => !step.done)?.action() ?? finish()
            }
          >
            {complete === steps.length ? "Start exploring" : "Continue setup"}
          </Button>
        </>
      }
    >
      <div className="mb-5 rounded-xl bg-gradient-to-br from-accent/15 to-transparent p-4">
        <div className="flex items-end justify-between">
          <div>
            <p className="text-sm font-semibold">First-launch setup</p>
            <p className="text-xs text-muted-foreground">
              {complete} of {steps.length} complete
            </p>
          </div>
          <span className="text-2xl font-bold text-accent">
            {Math.round((complete / steps.length) * 100)}%
          </span>
        </div>
        <div className="mt-3 h-2 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-accent transition-all"
            style={{ width: `${(complete / steps.length) * 100}%` }}
          />
        </div>
      </div>
      <div className="space-y-2">
        {steps.map((step) => {
          const Icon = step.icon;
          return (
            <button
              key={step.title}
              disabled={step.done}
              onClick={step.action}
              className="flex w-full items-center gap-3 rounded-xl border p-3 text-left transition enabled:hover:border-accent/40 enabled:hover:bg-accent/5"
            >
              <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted">
                <Icon className="h-4 w-4" />
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium">{step.title}</p>
                <p className="text-xs text-muted-foreground">{step.detail}</p>
              </div>
              {step.done ? (
                <span className="flex h-6 w-6 items-center justify-center rounded-full bg-emerald-500/15 text-emerald-400">
                  <Check className="h-3.5 w-3.5" />
                </span>
              ) : (
                <Circle className="h-5 w-5 text-muted-foreground" />
              )}
            </button>
          );
        })}
      </div>
    </Modal>
  );
}

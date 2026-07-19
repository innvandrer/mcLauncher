import { useState } from "react";
import { Layers, Loader2, Plus, Trash2 } from "lucide-react";
import { api, errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";
import type { Instance } from "@/lib/types";

/**
 * Named enable/disable mod sets for one instance ("Performance", "Building",
 * "Vanilla-ish for the server"…). Saving snapshots the current checkbox state;
 * applying toggles mods to match. The backend keys entries by project id, so
 * loadouts survive mod version updates.
 */
export function LoadoutsMenu({
  instance,
  onApplied,
}: {
  instance: Instance;
  /** Called after a loadout was applied so the mod list can refresh. */
  onApplied: () => void;
}) {
  const toast = useStore((s) => s.toast);
  const refreshInstances = useStore((s) => s.refreshInstances);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const loadouts = instance.loadouts ?? [];

  const apply = async (n: string) => {
    setBusy(n);
    try {
      const changed = await api.applyLoadout(instance.id, n);
      toast(
        "success",
        changed > 0
          ? `Applied “${n}” — ${changed} ${changed === 1 ? "mod" : "mods"} toggled`
          : `Applied “${n}” — already matching`,
      );
      onApplied();
      setOpen(false);
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const saveCurrent = async () => {
    const n = name.trim();
    if (!n) return;
    try {
      await api.saveLoadout(instance.id, n);
      await refreshInstances();
      toast("success", `Saved loadout “${n}”`);
      setName("");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const remove = async (n: string) => {
    try {
      await api.deleteLoadout(instance.id, n);
      await refreshInstances();
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className="inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus"
        title="Loadouts — named on/off mod sets"
      >
        <Layers className="h-3 w-3" />
        Loadouts
      </button>
      {open && (
        <>
          {/* Click-away backdrop (the menu contains an input, so blur-close won't work). */}
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-6 z-20 w-64 rounded-lg border bg-card py-1 shadow-xl animate-fade-in">
            {loadouts.length === 0 && (
              <p className="px-3 py-2 text-xs text-muted-foreground">
                Save the current on/off state of your mods as a named set, then
                switch between sets with one click.
              </p>
            )}
            {loadouts.map((l) => (
              <div
                key={l.name}
                className="group flex items-center gap-1 px-1.5 transition-colors hover:bg-muted"
              >
                <button
                  onClick={() => apply(l.name)}
                  disabled={busy !== null}
                  className="flex min-w-0 flex-1 flex-col gap-0.5 px-1.5 py-2 text-left disabled:opacity-60"
                  title={`Apply “${l.name}”`}
                >
                  <span className="flex items-center gap-1.5 truncate text-sm font-medium text-foreground">
                    {busy === l.name && (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    )}
                    {l.name}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {l.disabled.length === 0
                      ? "everything on"
                      : `${l.disabled.length} ${l.disabled.length === 1 ? "mod" : "mods"} off`}
                  </span>
                </button>
                <button
                  onClick={() => remove(l.name)}
                  className="rounded-md p-1.5 text-muted-foreground opacity-0 transition hover:text-red-400 group-hover:opacity-100"
                  title={`Delete “${l.name}”`}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
            <div className="mt-1 border-t px-3 py-2">
              <div className="flex items-center gap-1.5">
                <input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && saveCurrent()}
                  placeholder="Save current as…"
                  className="h-8 min-w-0 flex-1 rounded-md border bg-muted/40 px-2 text-sm outline-none placeholder:text-muted-foreground focus:border-accent"
                />
                <button
                  onClick={saveCurrent}
                  disabled={!name.trim()}
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-accent/15 text-accent transition hover:bg-accent/25 disabled:opacity-40 btn-focus"
                  title="Save loadout"
                >
                  <Plus className="h-4 w-4" />
                </button>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

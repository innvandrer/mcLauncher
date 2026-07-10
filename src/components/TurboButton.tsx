import { useState } from "react";
import { Loader2, Undo2, Zap } from "lucide-react";
import { api, errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";
import type { Instance, TurboResult } from "@/lib/types";

/**
 * One click installs a small, well-established performance mod stack
 * (Sodium/Lithium/FerriteCore-class) matched to the instance's loader.
 * Mods already present are skipped by the backend, not reinstalled.
 */
export function TurboButton({
  instance,
  onApplied,
}: {
  instance: Instance;
  /** Called after mods are installed or undone, so the mod list refreshes. */
  onApplied: () => void;
}) {
  const toast = useStore((s) => s.toast);
  const [running, setRunning] = useState(false);
  const [undoing, setUndoing] = useState(false);
  const [result, setResult] = useState<TurboResult | null>(null);

  if (instance.loader === "vanilla") return null;

  const apply = async () => {
    setRunning(true);
    setResult(null);
    try {
      const res = await api.applyTurbo(instance.id);
      setResult(res);
      if (res.installed.length > 0) {
        toast("success", `Turbo: installed ${res.installed.length} file${res.installed.length === 1 ? "" : "s"}`);
        onApplied();
      } else if (res.skipped.length > 0) {
        toast("info", "Turbo: everything's already installed");
      } else {
        toast("info", "No performance stack for this loader yet");
      }
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setRunning(false);
    }
  };

  const undo = async () => {
    if (!result || result.installed.length === 0) return;
    setUndoing(true);
    try {
      await Promise.all(result.installed.map((file) => api.deleteMod(instance.id, file)));
      toast("success", "Turbo changes undone");
      setResult(null);
      onApplied();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setUndoing(false);
    }
  };

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={apply}
        disabled={running}
        className="inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus disabled:opacity-50"
        title="Install a well-known performance mod stack (Sodium/Lithium-class)"
      >
        {running ? <Loader2 className="h-3 w-3 animate-spin" /> : <Zap className="h-3 w-3" />}
        Turbo
      </button>
      {result && result.installed.length > 0 && (
        <button
          onClick={undo}
          disabled={undoing}
          className="inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus disabled:opacity-50"
          title="Remove the mods Turbo just installed"
        >
          {undoing ? <Loader2 className="h-3 w-3 animate-spin" /> : <Undo2 className="h-3 w-3" />}
          Undo
        </button>
      )}
    </div>
  );
}

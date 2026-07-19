import { useEffect, useMemo, useState } from "react";
import { CheckCircle2, FlaskConical, Play, RotateCcw } from "lucide-react";
import { api, errMessage } from "@/lib/api";
import { recordActivity } from "@/lib/activity";
import type { Instance, ModEntry } from "@/lib/types";
import { useStore } from "@/store/useStore";
import { Button, Modal, Spinner } from "./ui";

const BACKUP_NAME = "__EZMapa Pack Doctor backup";
interface DoctorSession {
  candidates: string[];
  disabled: string[];
  pass: number;
}

export function PackDoctorModal({
  open,
  onClose,
  instance,
}: {
  open: boolean;
  onClose: () => void;
  instance: Instance;
}) {
  const toast = useStore((state) => state.toast);
  const refreshInstances = useStore((state) => state.refreshInstances);
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const storageKey = `ezmapa:doctor:${instance.id}`;
  const [session, setSession] = useState<DoctorSession | null>(() => {
    try {
      return JSON.parse(localStorage.getItem(storageKey) || "null");
    } catch {
      return null;
    }
  });
  useEffect(() => {
    if (open)
      api
        .listMods(instance.id)
        .then(setMods)
        .catch(() => {});
  }, [open, instance.id]);
  const enabled = useMemo(
    () => mods.filter((mod) => mod.enabled).map((mod) => mod.fileName),
    [mods],
  );
  const persist = (next: DoctorSession | null) => {
    setSession(next);
    if (next) localStorage.setItem(storageKey, JSON.stringify(next));
    else localStorage.removeItem(storageKey);
  };

  const applyPass = async (candidates: string[], pass: number) => {
    await api.applyLoadout(instance.id, BACKUP_NAME);
    const disabled = candidates.slice(
      0,
      Math.max(1, Math.ceil(candidates.length / 2)),
    );
    for (const fileName of disabled)
      await api.setModEnabled(instance.id, fileName, false);
    const next = { candidates, disabled, pass };
    persist(next);
    await refreshInstances();
    recordActivity({
      kind: "content",
      instanceId: instance.id,
      message: `Pack Doctor pass ${pass} prepared`,
      detail: `${disabled.length} mods disabled; ${candidates.length} suspects remain`,
    });
    return next;
  };
  const start = async () => {
    setBusy(true);
    try {
      await api.saveLoadout(instance.id, BACKUP_NAME);
      await applyPass(enabled, 1);
      toast(
        "info",
        "Isolation pass ready. Test-launch, then report whether the crash remains.",
      );
    } catch (error) {
      toast("error", errMessage(error));
    } finally {
      setBusy(false);
    }
  };
  const report = async (crashRemained: boolean) => {
    if (!session) return;
    setBusy(true);
    try {
      const disabledSet = new Set(session.disabled);
      const candidates = crashRemained
        ? session.candidates.filter((name) => !disabledSet.has(name))
        : session.candidates.filter((name) => disabledSet.has(name));
      if (candidates.length <= 1) {
        await api.applyLoadout(instance.id, BACKUP_NAME);
        const done = { candidates, disabled: [], pass: session.pass + 1 };
        persist(done);
        recordActivity({
          kind: "crash",
          instanceId: instance.id,
          message: candidates.length
            ? `Pack Doctor isolated ${candidates[0]}`
            : "Pack Doctor found no consistent suspect",
        });
        await refreshInstances();
      } else {
        await applyPass(candidates, session.pass + 1);
      }
    } catch (error) {
      toast("error", errMessage(error));
    } finally {
      setBusy(false);
    }
  };
  const restore = async () => {
    setBusy(true);
    try {
      await api.applyLoadout(instance.id, BACKUP_NAME);
      await api.deleteLoadout(instance.id, BACKUP_NAME);
      persist(null);
      toast("success", "Original mod state restored");
      await refreshInstances();
      onClose();
    } catch (error) {
      toast("error", errMessage(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} title="Pack Doctor" size="md">
      {!mods.length ? (
        <div className="flex justify-center py-10">
          <Spinner />
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex gap-3 rounded-xl border border-amber-500/30 bg-amber-500/10 p-3">
            <FlaskConical className="h-5 w-5 shrink-0 text-amber-400" />
            <div>
              <p className="text-sm font-semibold">Guided binary isolation</p>
              <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                EZMapa saves the original loadout, tests half the suspects at a
                time, and resumes this session even after restarting the
                launcher.
              </p>
            </div>
          </div>
          {!session ? (
            <>
              <p className="text-sm">
                Ready to analyze <strong>{enabled.length}</strong> enabled mods.
                Recent changes are tested first where possible.
              </p>
              <div className="flex justify-end">
                <Button variant="primary" onClick={start} loading={busy}>
                  Start isolation
                </Button>
              </div>
            </>
          ) : session.candidates.length <= 1 ? (
            <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-center">
              <CheckCircle2 className="mx-auto h-8 w-8 text-emerald-400" />
              <p className="mt-2 font-semibold">
                {session.candidates[0]
                  ? "Likely culprit isolated"
                  : "No consistent culprit"}
              </p>
              {session.candidates[0] && (
                <p className="mt-1 break-all rounded-lg bg-background/40 p-2 font-mono text-xs">
                  {session.candidates[0]}
                </p>
              )}
              <Button
                className="mt-4"
                variant="secondary"
                onClick={restore}
                loading={busy}
              >
                <RotateCcw className="h-4 w-4" />
                Restore original state
              </Button>
            </div>
          ) : (
            <>
              <div className="rounded-xl border p-4">
                <div className="flex items-center justify-between">
                  <p className="font-semibold">Pass {session.pass}</p>
                  <span className="rounded-full bg-accent/15 px-2.5 py-1 text-xs text-accent">
                    {session.candidates.length} suspects
                  </span>
                </div>
                <p className="mt-2 text-sm text-muted-foreground">
                  {session.disabled.length} mods are temporarily disabled.
                  Launch the game, reproduce the problem, then record what
                  happened.
                </p>
                <Button
                  className="mt-3"
                  variant="secondary"
                  onClick={() => api.launchInstance(instance.id)}
                >
                  <Play className="h-4 w-4" />
                  Test launch
                </Button>
              </div>
              <div className="grid gap-2 sm:grid-cols-2">
                <Button
                  variant="danger"
                  disabled={busy}
                  onClick={() => report(true)}
                >
                  Crash remained
                </Button>
                <Button
                  variant="primary"
                  disabled={busy}
                  onClick={() => report(false)}
                >
                  Crash disappeared
                </Button>
              </div>
              <div className="flex justify-end">
                <Button variant="ghost" onClick={restore} disabled={busy}>
                  <RotateCcw className="h-4 w-4" />
                  Cancel and restore
                </Button>
              </div>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}

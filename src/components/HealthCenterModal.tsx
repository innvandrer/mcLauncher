import { useEffect, useMemo, useState } from "react";
import { Activity, CheckCircle2, RotateCcw, ShieldAlert } from "lucide-react";
import { api, errMessage } from "@/lib/api";
import type {
  DiskUsage,
  Instance,
  ModUpdate,
  PreflightWarning,
  Snapshot,
} from "@/lib/types";
import { formatBytes } from "@/lib/utils";
import { healthScore } from "@/lib/diagnostics";
import { useStore } from "@/store/useStore";
import { Button, Modal, Spinner } from "./ui";
import { useActivities } from "@/lib/activity";

export function HealthCenterModal({
  open,
  onClose,
  instance,
}: {
  open: boolean;
  onClose: () => void;
  instance: Instance;
}) {
  const toast = useStore((state) => state.toast);
  const [loading, setLoading] = useState(false);
  const [warnings, setWarnings] = useState<PreflightWarning[]>([]);
  const [updates, setUpdates] = useState<ModUpdate[]>([]);
  const [usage, setUsage] = useState<DiskUsage | null>(null);
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [rollingBack, setRollingBack] = useState(false);
  const history = useActivities()
    .filter(
      (entry) =>
        (entry.instanceId === instance.id || !entry.instanceId) &&
        (entry.kind === "content" || entry.kind === "backup"),
    )
    .slice(0, 5);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    Promise.all([
      api.preflightCheck(instance.id),
      api.checkModUpdates({
        instanceId: instance.id,
        loader: instance.loader,
        gameVersion: instance.mcVersion,
      }),
      api.instanceDiskUsage(instance.id),
      api.listSnapshots(instance.id),
    ])
      .then(([nextWarnings, nextUpdates, nextUsage, nextSnapshots]) => {
        setWarnings(nextWarnings);
        setUpdates(nextUpdates);
        setUsage(nextUsage);
        setSnapshots(nextSnapshots);
      })
      .catch((error) => toast("error", errMessage(error)))
      .finally(() => setLoading(false));
  }, [open, instance, toast]);

  const score = useMemo(
    () => healthScore(warnings.length, updates.length),
    [warnings.length, updates.length],
  );

  const rollback = async () => {
    setRollingBack(true);
    try {
      const count = await api.rollbackLastContentUpdate(instance.id);
      toast("success", `Restored ${count} item${count === 1 ? "" : "s"}`);
      onClose();
    } catch (error) {
      toast("error", errMessage(error));
    } finally {
      setRollingBack(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Instance Health Center"
      size="lg"
    >
      {loading ? (
        <div className="flex items-center justify-center gap-2 py-12 text-sm text-muted-foreground">
          <Spinner /> Running local health checks…
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex items-center gap-4 rounded-xl border bg-muted/30 p-4">
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-accent/15 text-2xl font-bold text-accent">
              {score}
            </div>
            <div>
              <p className="font-semibold">
                {score >= 90
                  ? "Ready to play"
                  : score >= 65
                    ? "A few things need attention"
                    : "Needs attention"}
              </p>
              <p className="text-sm text-muted-foreground">
                {warnings.length} warnings · {updates.length} updates ·{" "}
                {usage ? formatBytes(usage.total) : "unknown size"}
              </p>
            </div>
          </div>

          <HealthSection
            title="Preflight"
            healthy={warnings.length === 0}
            detail={
              warnings.length === 0
                ? "Java, memory, and duplicate-mod checks passed."
                : warnings.map((warning) => warning.title).join(" · ")
            }
          />

          <section className="rounded-xl border p-3">
            <div className="mb-2 flex items-center gap-2">
              <Activity className="h-4 w-4 text-accent" />
              <p className="text-sm font-medium">Change & recovery timeline</p>
            </div>
            {history.length ? (
              <div className="space-y-1">
                {history.map((entry) => (
                  <div
                    key={entry.id}
                    className="flex items-center justify-between gap-4 rounded-lg bg-muted/30 px-3 py-2"
                  >
                    <div className="min-w-0">
                      <p className="truncate text-xs font-medium">
                        {entry.message}
                      </p>
                      {entry.detail && (
                        <p className="truncate text-[11px] text-muted-foreground">
                          {entry.detail}
                        </p>
                      )}
                    </div>
                    <span className="shrink-0 text-[10px] text-muted-foreground">
                      {new Date(entry.created).toLocaleDateString()}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">
                The next update or backup will appear here with a rollback
                point.
              </p>
            )}
          </section>
          <HealthSection
            title="Content"
            healthy={updates.length === 0}
            detail={
              updates.length === 0
                ? "Installed content is current."
                : `${updates.length} reviewed update${updates.length === 1 ? "" : "s"} available.`
            }
          />
          <HealthSection
            title="Recovery"
            healthy={snapshots.length > 0}
            detail={
              snapshots.length > 0
                ? `${snapshots.length} world snapshot${snapshots.length === 1 ? "" : "s"} available.`
                : "No world snapshots yet."
            }
          />

          <div className="flex flex-wrap justify-end gap-2 border-t pt-4">
            <Button
              variant="secondary"
              onClick={rollback}
              loading={rollingBack}
            >
              <RotateCcw className="h-4 w-4" /> Roll back last content update
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

function HealthSection({
  title,
  healthy,
  detail,
}: {
  title: string;
  healthy: boolean;
  detail: string;
}) {
  const Icon = healthy ? CheckCircle2 : ShieldAlert;
  return (
    <div className="flex items-start gap-3 rounded-lg border p-3">
      <Icon
        className={
          healthy ? "h-5 w-5 text-emerald-400" : "h-5 w-5 text-amber-400"
        }
      />
      <div>
        <p className="text-sm font-medium">{title}</p>
        <p className="text-xs text-muted-foreground">{detail}</p>
      </div>
      <Activity className="ml-auto h-4 w-4 text-muted-foreground/40" />
    </div>
  );
}

import { useEffect, useState } from "react";
import {
  AlertTriangle,
  Check,
  ExternalLink,
  HelpCircle,
  MinusCircle,
} from "lucide-react";
import { Button, Field, Input, Modal, Spinner } from "./ui";
import { api, errMessage } from "@/lib/api";
import { t } from "@/lib/strings";
import { useStore } from "@/store/useStore";
import type { ServerModPlan } from "@/lib/types";

/**
 * "Create matching instance" flow for modded (Forge/NeoForge) servers:
 * analyzes the server's decoded mod list, shows what can be installed
 * automatically vs. what needs manual work, and builds the instance on
 * confirm. Clearly labeled best-effort.
 */
export function ServerInstanceModal({
  open,
  address,
  defaultName,
  onClose,
}: {
  open: boolean;
  address: string;
  defaultName: string;
  onClose: () => void;
}) {
  const toast = useStore((s) => s.toast);
  const [plan, setPlan] = useState<ServerModPlan | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState(defaultName);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setPlan(null);
    setError(null);
    setName(defaultName);
    setAnalyzing(true);
    api
      .analyzeServerMods(address)
      .then(setPlan)
      .catch((e) => setError(errMessage(e)))
      .finally(() => setAnalyzing(false));
  }, [open, address, defaultName]);

  const create = async () => {
    if (!plan) return;
    setCreating(true);
    try {
      await api.createInstanceFromServer(name.trim() || defaultName, address);
      toast("success", t("server.created"));
      onClose();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setCreating(false);
    }
  };

  const total = plan
    ? plan.resolved.length + plan.unresolved.length + plan.skipped.length
    : 0;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t("server.buildInstance")}
      size="lg"
    >
      <div className="space-y-4">
        <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-300">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          {t("server.bestEffort")}
        </div>

        {analyzing && (
          <div className="flex items-center gap-2 py-6 text-sm text-muted-foreground">
            <Spinner className="h-4 w-4" />
            {t("server.analyzing")}
          </div>
        )}

        {error && (
          <p className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </p>
        )}

        {plan && (
          <>
            <p className="text-sm text-muted-foreground">
              {plan.loader} · {plan.mcVersion ?? "?"} ·{" "}
              {t("server.planSummary", {
                resolved: plan.resolved.length,
                total,
                unresolved: plan.unresolved.length,
                skipped: plan.skipped.length,
              })}
            </p>
            {plan.truncated && (
              <p className="text-xs text-amber-400">{t("server.truncated")}</p>
            )}

            {plan.resolved.length > 0 && (
              <section>
                <h4 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  {t("server.resolvedTitle")} ({plan.resolved.length})
                </h4>
                <ul className="max-h-40 space-y-1 overflow-y-auto pr-1 text-sm">
                  {plan.resolved.map((m) => (
                    <li key={m.modId} className="flex items-center gap-2">
                      <Check className="h-3.5 w-3.5 shrink-0 text-emerald-400" />
                      <span className="min-w-0 truncate">{m.fileName}</span>
                      {!m.exact && (
                        <span className="shrink-0 rounded bg-amber-500/15 px-1.5 py-px text-[10px] text-amber-400">
                          {t("server.approx")}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {plan.unresolved.length > 0 && (
              <section>
                <h4 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  {t("server.unresolvedTitle")} ({plan.unresolved.length})
                </h4>
                <ul className="max-h-32 space-y-1 overflow-y-auto pr-1 text-sm">
                  {plan.unresolved.map((m) => (
                    <li key={m.modId} className="flex items-center gap-2">
                      <HelpCircle className="h-3.5 w-3.5 shrink-0 text-amber-400" />
                      <span className="min-w-0 truncate">
                        {m.modId}{" "}
                        <span className="text-muted-foreground">
                          ({m.version})
                        </span>
                      </span>
                      <button
                        onClick={() => api.openUrl(m.searchUrl)}
                        className="inline-flex shrink-0 items-center gap-1 text-xs text-accent hover:underline btn-focus"
                      >
                        <ExternalLink className="h-3 w-3" />
                        {t("server.searchLink")}
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {plan.skipped.length > 0 && (
              <section>
                <h4 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  {t("server.skippedTitle")} ({plan.skipped.length})
                </h4>
                <p className="line-clamp-2 text-xs text-muted-foreground">
                  <MinusCircle className="mr-1 inline h-3 w-3" />
                  {plan.skipped.map((s) => s.modId).join(", ")}
                </p>
              </section>
            )}

            <Field label={t("server.nameLabel")}>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </Field>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={onClose}>
                {t("export.cancel")}
              </Button>
              <Button
                variant="primary"
                loading={creating}
                disabled={
                  plan.resolved.length === 0 && plan.unresolved.length === 0
                }
                onClick={create}
              >
                {t("server.create")}
              </Button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}

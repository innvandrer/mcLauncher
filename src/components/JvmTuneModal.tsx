import { useEffect, useMemo, useState } from "react";
import { Button, Modal, Spinner } from "./ui";
import { api, errMessage } from "@/lib/api";
import { t } from "@/lib/strings";
import { cn } from "@/lib/utils";
import type { JvmSuggestion } from "@/lib/types";

/**
 * "Recommended JVM settings" dialog: shows the suggested heap + GC flags as a
 * before/after diff (token-level) and only hands the values back to the
 * settings form when the user confirms. When the instance has custom args the
 * suggestion is a merge (non-GC arguments kept) and says so.
 */
export function JvmTuneModal({
  open,
  instanceId,
  onClose,
  onApply,
}: {
  open: boolean;
  instanceId: string;
  onClose: () => void;
  /** Receives the confirmed (args, xmxMb) to fill into the settings form. */
  onApply: (args: string, xmxMb: number) => void;
}) {
  const [suggestion, setSuggestion] = useState<JvmSuggestion | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open) return;
    setSuggestion(null);
    setError(null);
    setLoading(true);
    api
      .suggestJvmArgs(instanceId)
      .then(setSuggestion)
      .catch((e) => setError(errMessage(e)))
      .finally(() => setLoading(false));
  }, [open, instanceId]);

  const diff = useMemo(() => {
    if (!suggestion)
      return { removed: new Set<string>(), added: new Set<string>() };
    const current = new Set(
      suggestion.currentArgs.split(/\s+/).filter(Boolean),
    );
    const next = new Set(suggestion.suggestedArgs.split(/\s+/).filter(Boolean));
    return {
      removed: new Set([...current].filter((x) => !next.has(x))),
      added: new Set([...next].filter((x) => !current.has(x))),
    };
  }, [suggestion]);

  const tokens = (
    args: string,
    highlight: Set<string>,
    tone: "removed" | "added",
  ) => (
    <div className="flex flex-wrap gap-1 rounded-lg border bg-muted/30 p-2 font-mono text-[11px] leading-5">
      {args
        .split(/\s+/)
        .filter(Boolean)
        .map((tok, i) => (
          <span
            key={`${tok}-${i}`}
            className={cn(
              "rounded px-1",
              highlight.has(tok)
                ? tone === "removed"
                  ? "bg-red-500/15 text-red-400 line-through"
                  : "bg-emerald-500/15 text-emerald-400"
                : "text-muted-foreground",
            )}
          >
            {tok}
          </span>
        ))}
      {args.trim() === "" && (
        <span className="text-muted-foreground/60">(empty)</span>
      )}
    </div>
  );

  return (
    <Modal open={open} onClose={onClose} title={t("jvm.title")} size="lg">
      <div className="space-y-4">
        {loading && (
          <div className="flex items-center gap-2 py-6 text-sm text-muted-foreground">
            <Spinner className="h-4 w-4" />
          </div>
        )}
        {error && (
          <p className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </p>
        )}

        {suggestion && (
          <>
            <ul className="list-inside list-disc space-y-0.5 text-xs text-muted-foreground">
              {suggestion.reasons.map((r) => (
                <li key={r}>{r}</li>
              ))}
            </ul>

            <div>
              <h4 className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t("jvm.memory")}
              </h4>
              <p className="text-sm">
                <span
                  className={cn(
                    suggestion.currentXmxMb !== suggestion.suggestedXmxMb &&
                      "text-red-400 line-through",
                  )}
                >
                  {suggestion.currentXmxMb} MB
                </span>
                {suggestion.currentXmxMb !== suggestion.suggestedXmxMb && (
                  <span className="ml-2 font-medium text-emerald-400">
                    {suggestion.suggestedXmxMb} MB
                  </span>
                )}
              </p>
            </div>

            {suggestion.hasCustomArgs && (
              <p className="rounded-lg border border-amber-500/40 bg-amber-500/10 p-2.5 text-xs text-amber-300">
                {t("jvm.mergeNote")}
              </p>
            )}

            <div className="space-y-2">
              <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t("jvm.argsDiff")}
              </h4>
              <div>
                <p className="mb-1 text-[11px] text-muted-foreground">
                  {t("jvm.currentLabel")}
                </p>
                {tokens(suggestion.currentArgs, diff.removed, "removed")}
              </div>
              <div>
                <p className="mb-1 text-[11px] text-muted-foreground">
                  {t("jvm.suggestedLabel")}
                </p>
                {tokens(suggestion.suggestedArgs, diff.added, "added")}
              </div>
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={onClose}>
                {t("export.cancel")}
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  onApply(suggestion.suggestedArgs, suggestion.suggestedXmxMb);
                  onClose();
                }}
              >
                {t("jvm.apply")}
              </Button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}

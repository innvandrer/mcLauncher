import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { AlertTriangle } from "lucide-react";
import { Button, Modal } from "./ui";
import { api, errMessage } from "@/lib/api";
import { t } from "@/lib/strings";
import { useStore } from "@/store/useStore";
import type { PackPreviewEntry } from "@/lib/types";

export type PackFormat = "mrpack" | "cfpack" | "both";

/** Does this file need an embed/exclude decision for the given format? */
function needsDecision(entry: PackPreviewEntry, format: PackFormat): boolean {
  switch (format) {
    case "mrpack":
      return (
        entry.availability === "curseforge" || entry.availability === "none"
      );
    case "cfpack":
      return entry.availability === "modrinth" || entry.availability === "none";
    case "both":
      return entry.availability !== "both";
  }
}

function missingLabel(entry: PackPreviewEntry): string {
  if (entry.availability === "none") return t("export.missingEverywhere");
  if (entry.availability === "modrinth") return t("export.missingOnCurseforge");
  return t("export.missingOnModrinth");
}

async function pickPaths(
  instanceName: string,
  format: PackFormat,
): Promise<{ mrpack?: string; cfpack?: string } | null> {
  const out: { mrpack?: string; cfpack?: string } = {};
  if (format === "mrpack" || format === "both") {
    const p = await save({
      defaultPath: `${instanceName}.mrpack`,
      filters: [{ name: "Modrinth modpack", extensions: ["mrpack"] }],
    });
    if (!p) return null;
    out.mrpack = p;
  }
  if (format === "cfpack" || format === "both") {
    const p = await save({
      defaultPath: `${instanceName}-curseforge.zip`,
      filters: [{ name: "CurseForge modpack", extensions: ["zip"] }],
    });
    if (!p) return null;
    out.cfpack = p;
  }
  return out;
}

async function runExports(
  instanceId: string,
  format: PackFormat,
  dest: { mrpack?: string; cfpack?: string },
  embedList: string[],
) {
  const store = useStore.getState();
  store.setPackExporting(true);
  try {
    store.toast("info", t("export.exporting"));
    if (dest.mrpack) await api.exportMrpack(instanceId, dest.mrpack, embedList);
    if (dest.cfpack)
      await api.exportCurseforgePack(instanceId, dest.cfpack, embedList);
    store.toast(
      "success",
      t(
        format === "both"
          ? "export.doneBoth"
          : format === "cfpack"
            ? "export.doneCfpack"
            : "export.doneMrpack",
      ),
    );
  } catch (e) {
    store.toast("error", errMessage(e));
  } finally {
    useStore.getState().setPackExporting(false);
  }
}

/**
 * Start a modpack export from anywhere (menus, command palette): pick the
 * destination(s), resolve every file on both platforms, then either export
 * straight away or open the global embed/exclude review dialog
 * ([`PackExportModal`], mounted once in App) for platform-exclusive files.
 */
export async function startPackExport(
  instanceId: string,
  instanceName: string,
  format: PackFormat,
) {
  const store = useStore.getState();
  try {
    const dest = await pickPaths(instanceName, format);
    if (!dest) return;
    const preview = await api.preparePackExport(instanceId);
    const undecided = preview.entries.filter((e) => needsDecision(e, format));
    if (undecided.length === 0) {
      await runExports(instanceId, format, dest, []);
      return;
    }
    store.setPackExport({
      instanceId,
      instanceName,
      format,
      entries: undecided,
      paths: dest,
    });
  } catch (e) {
    store.toast("error", errMessage(e));
  }
}

/**
 * The embed/exclude review dialog for platform-exclusive files. Defaults every
 * file to "exclude" — the user opts in per file (or via "embed all") after the
 * license warning.
 */
export function PackExportModal() {
  const pending = useStore((s) => s.packExport);
  const setPending = useStore((s) => s.setPackExport);
  const exporting = useStore((s) => s.packExporting);
  const [embed, setEmbed] = useState<Record<string, boolean>>({});

  // Reset the checkboxes (all excluded) whenever a new review opens.
  useEffect(() => {
    if (pending) {
      setEmbed(
        Object.fromEntries(pending.entries.map((e) => [e.fileName, false])),
      );
    }
  }, [pending]);

  const confirm = async () => {
    if (!pending) return;
    const embedList = Object.entries(embed)
      .filter(([, v]) => v)
      .map(([k]) => k);
    setPending(null);
    await runExports(
      pending.instanceId,
      pending.format,
      pending.paths,
      embedList,
    );
  };

  const allEmbedded =
    !!pending && pending.entries.every((e) => embed[e.fileName]);

  return (
    <Modal
      open={!!pending}
      onClose={() => setPending(null)}
      title={t("export.reviewTitle")}
      size="md"
    >
      {pending && (
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            {t("export.reviewIntro")}
          </p>

          <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-300">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            {t("export.licenseWarning")}
          </div>

          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={allEmbedded}
              onChange={(e) =>
                setEmbed(
                  Object.fromEntries(
                    pending.entries.map((en) => [
                      en.fileName,
                      e.target.checked,
                    ]),
                  ),
                )
              }
            />
            {t("export.embedAll")}
          </label>

          <ul className="max-h-64 space-y-1.5 overflow-y-auto pr-1">
            {pending.entries.map((e) => (
              <li
                key={`${e.subdir}/${e.fileName}`}
                className="flex items-center justify-between gap-3 rounded-lg border bg-muted/30 px-3 py-2"
              >
                <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-2">
                  <input
                    type="checkbox"
                    checked={embed[e.fileName] ?? false}
                    onChange={(ev) =>
                      setEmbed((m) => ({
                        ...m,
                        [e.fileName]: ev.target.checked,
                      }))
                    }
                  />
                  <span className="truncate text-sm">{e.fileName}</span>
                </label>
                <span className="shrink-0 rounded bg-muted px-1.5 py-px text-[10px] font-medium text-muted-foreground">
                  {missingLabel(e)}
                </span>
              </li>
            ))}
          </ul>

          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setPending(null)}>
              {t("export.cancel")}
            </Button>
            <Button variant="primary" loading={exporting} onClick={confirm}>
              {t("export.confirm")}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

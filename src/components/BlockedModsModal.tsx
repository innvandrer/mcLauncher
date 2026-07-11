import { ExternalLink, FolderOpen } from "lucide-react";
import { Button, Modal } from "./ui";
import { api } from "@/lib/api";
import { plural, t } from "@/lib/strings";
import { useStore } from "@/store/useStore";

/**
 * Shown after a CurseForge modpack install when some files couldn't be
 * downloaded automatically (author disabled third-party distribution and no
 * identical file exists on Modrinth). Lists each file with a link to its
 * CurseForge page so the user can finish the install manually.
 */
export function BlockedModsModal() {
  const activeId = useStore((s) => s.activePackReportId);
  const report = useStore((s) => (s.activePackReportId ? s.packReports[s.activePackReportId] : undefined));
  const dismiss = useStore((s) => s.dismissPackReport);

  const open = !!activeId && !!report && report.blocked.length > 0;
  const close = () => activeId && dismiss(activeId);

  return (
    <Modal open={open} onClose={close} title={t("blocked.title")} size="md">
      {report && (
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            {t("blocked.summary", {
              n: report.blocked.length,
              mods: plural(report.blocked.length, "blocked.mod", "blocked.mods"),
              authors: plural(report.blocked.length, "blocked.author", "blocked.authors"),
            })}{" "}
            {t("blocked.instruction")}
          </p>

          <ul className="max-h-64 space-y-1.5 overflow-y-auto pr-1">
            {report.blocked.map((b) => (
              <li
                key={b.fileId}
                className="flex items-center justify-between gap-3 rounded-lg border bg-muted/30 px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium">{b.modName}</div>
                  <div className="truncate text-xs text-muted-foreground">{b.fileName}</div>
                </div>
                <Button
                  variant="ghost"
                  className="shrink-0"
                  onClick={() => api.openUrl(b.pageUrl)}
                  title={b.pageUrl}
                >
                  <ExternalLink className="h-4 w-4" />
                  {t("blocked.openPage")}
                </Button>
              </li>
            ))}
          </ul>

          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => api.openInstanceFolder(report.instanceId)}>
              <FolderOpen className="h-4 w-4" />
              {t("blocked.openFolder")}
            </Button>
            <Button variant="primary" onClick={close}>
              {t("blocked.dismiss")}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

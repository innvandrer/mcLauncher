import { useState } from "react";
import { ChevronDown, Loader2, Upload } from "lucide-react";
import { cn } from "@/lib/utils";
import { exportInstanceZip } from "@/lib/export";
import { errMessage } from "@/lib/api";
import { t } from "@/lib/strings";
import { startPackExport, type PackFormat } from "./PackExport";
import { useStore } from "@/store/useStore";
import { save } from "@tauri-apps/plugin-dialog";
import { api } from "@/lib/api";

type ExportFormat = "zip" | PackFormat;

interface ExportMenuProps {
  instanceId: string;
  instanceName: string;
  /** Compact icon-only trigger for instance cards. */
  compact?: boolean;
  className?: string;
  onExportStart?: (format: ExportFormat) => void;
  onExportEnd?: () => void;
}

export function ExportMenu({
  instanceId,
  instanceName,
  compact,
  className,
  onExportStart,
  onExportEnd,
}: ExportMenuProps) {
  const toast = useStore((s) => s.toast);
  const packExporting = useStore((s) => s.packExporting);
  const [open, setOpen] = useState(false);
  const [zipping, setZipping] = useState(false);
  const exporting = zipping || packExporting;

  const runZipExport = async () => {
    setOpen(false);
    setZipping(true);
    onExportStart?.("zip");
    try {
      toast("info", "Exporting instance…");
      const path = await exportInstanceZip(instanceId, instanceName);
      if (path) toast("success", "Instance exported");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setZipping(false);
      onExportEnd?.();
    }
  };

  const runPackExport = async (format: PackFormat) => {
    setOpen(false);
    onExportStart?.(format);
    await startPackExport(instanceId, instanceName, format);
    onExportEnd?.();
  };

  const runShareExport = async () => {
    setOpen(false);
    const path = await save({
      defaultPath: `${instanceName}.ezmapa`,
      filters: [{ name: "EZMapa share manifest", extensions: ["ezmapa"] }],
    });
    if (!path) return;
    try {
      await api.exportShareManifest(instanceId, path);
      toast("success", "Share file exported");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  return (
    <div className={cn("relative", className)}>
      <button
        type="button"
        disabled={exporting}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        title="Export instance"
        className={cn(
          "inline-flex items-center justify-center gap-2 rounded-lg font-medium btn-focus transition-all",
          "disabled:opacity-50 disabled:pointer-events-none active:scale-[0.98]",
          compact
            ? "h-9 w-full px-3 text-sm text-foreground hover:bg-muted"
            : "h-10 bg-muted/70 px-4 text-sm text-foreground hover:bg-muted",
        )}
      >
        {exporting ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <Upload className="h-4 w-4" />
        )}
        {!compact && (
          <>
            Export
            <ChevronDown className="h-3.5 w-3.5 opacity-60" />
          </>
        )}
        {compact && "Export…"}
      </button>
      {open && (
        <div
          className={cn(
            "absolute z-20 overflow-hidden rounded-lg border bg-card py-1 shadow-xl animate-fade-in",
            compact ? "left-0 top-full mt-1 w-52" : "right-0 top-11 w-56",
          )}
          onMouseDown={(e) => e.preventDefault()}
        >
          <MenuItem
            label="Export as .zip"
            hint="Full EZMapa instance backup"
            onClick={runZipExport}
          />
          <MenuItem
            label="Pass the Pack (.ezmapa)"
            hint="Tiny manifest that re-downloads the instance"
            onClick={runShareExport}
          />
          <MenuItem
            label="Export as .mrpack"
            hint="Modrinth modpack for sharing"
            onClick={() => runPackExport("mrpack")}
          />
          <MenuItem
            label={t("export.cfpack")}
            hint={t("export.cfpackHint")}
            onClick={() => runPackExport("cfpack")}
          />
          <MenuItem
            label={t("export.both")}
            hint={t("export.bothHint")}
            onClick={() => runPackExport("both")}
          />
        </div>
      )}
    </div>
  );
}

function MenuItem({
  label,
  hint,
  onClick,
}: {
  label: string;
  hint: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className="flex w-full flex-col gap-0.5 px-3 py-2 text-left transition-colors hover:bg-muted"
    >
      <span className="text-sm font-medium text-foreground">{label}</span>
      <span className="text-xs text-muted-foreground">{hint}</span>
    </button>
  );
}

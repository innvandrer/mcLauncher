import { useState } from "react";
import { ChevronDown, Loader2, Upload } from "lucide-react";
import { cn } from "@/lib/utils";
import { exportInstanceMrpack, exportInstanceZip } from "@/lib/export";
import { errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";

type ExportFormat = "zip" | "mrpack";

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
  const [open, setOpen] = useState(false);
  const [exporting, setExporting] = useState<ExportFormat | null>(null);

  const runExport = async (format: ExportFormat) => {
    setOpen(false);
    setExporting(format);
    onExportStart?.(format);
    try {
      toast("info", format === "zip" ? "Exporting instance…" : "Exporting modpack…");
      const path =
        format === "zip"
          ? await exportInstanceZip(instanceId, instanceName)
          : await exportInstanceMrpack(instanceId, instanceName);
      if (path) {
        toast("success", format === "zip" ? "Instance exported" : "Exported .mrpack");
      }
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setExporting(null);
      onExportEnd?.();
    }
  };

  return (
    <div className={cn("relative", className)}>
      <button
        type="button"
        disabled={!!exporting}
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
            onClick={() => runExport("zip")}
          />
          <MenuItem
            label="Export as .mrpack"
            hint="Modrinth modpack for sharing"
            onClick={() => runExport("mrpack")}
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

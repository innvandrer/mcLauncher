import { useEffect, useState } from "react";
import {
  CheckCircle2,
  Clipboard,
  Link2,
  Package,
  TriangleAlert,
} from "lucide-react";
import { api, errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";
import { Button, Modal, Spinner } from "@/components/ui";
import type { Instance } from "@/lib/types";

export function SharePreviewModal({
  open,
  onClose,
  instance,
}: {
  open: boolean;
  onClose: () => void;
  instance: Instance;
}) {
  const toast = useStore((s) => s.toast);
  const [counts, setCounts] = useState<{
    mods: number;
    packs: number;
    shaders: number;
    tracked: number;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!open) return;
    setCounts(null);
    Promise.all([
      api.listMods(instance.id),
      api.listResourcePacks(instance.id),
      api.listShaders(instance.id),
      api.preparePackExport(instance.id),
    ])
      .then(([mods, packs, shaders, preview]) =>
        setCounts({
          mods: mods.length,
          packs: packs.length,
          shaders: shaders.length,
          tracked: preview.entries.filter(
            (entry) => entry.availability !== "none",
          ).length,
        }),
      )
      .catch(() => setCounts({ mods: 0, packs: 0, shaders: 0, tracked: 0 }));
  }, [instance.id, open]);
  const copyCode = async () => {
    setBusy(true);
    try {
      const json = await api.getShareCode(instance.id);
      const bytes = new TextEncoder().encode(json);
      let binary = "";
      bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
      await navigator.clipboard.writeText(`EZMAPA1:${btoa(binary)}`);
      toast("success", "Pass the Pack code copied");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const total = counts ? counts.mods + counts.packs + counts.shaders : 0;
  const manual = counts ? Math.max(0, total - counts.tracked) : 0;
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Pass the Pack preview"
      size="md"
    >
      {!counts ? (
        <div className="flex justify-center py-12">
          <Spinner />
        </div>
      ) : (
        <div className="space-y-4">
          <div className="rounded-xl border bg-gradient-to-br from-accent/15 to-transparent p-4">
            <div className="flex items-center gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-accent/15 text-accent">
                <Package className="h-5 w-5" />
              </div>
              <div>
                <p className="font-semibold">{instance.name}</p>
                <p className="text-xs text-muted-foreground">
                  Minecraft {instance.mcVersion} · {instance.loader}
                </p>
              </div>
            </div>
            <div className="mt-4 grid grid-cols-3 gap-2 text-center">
              <Count value={counts.mods} label="Mods" />
              <Count value={counts.packs} label="Packs" />
              <Count value={counts.shaders} label="Shaders" />
            </div>
          </div>
          <div className="space-y-2">
            <div className="flex gap-3 rounded-xl border border-emerald-500/25 bg-emerald-500/10 p-3">
              <CheckCircle2 className="mt-0.5 h-4 w-4 text-emerald-400" />
              <div>
                <p className="text-sm font-medium">
                  {counts.tracked} items install automatically
                </p>
                <p className="text-xs text-muted-foreground">
                  Recipients download these directly from their original
                  provider.
                </p>
              </div>
            </div>
            {manual > 0 && (
              <div className="flex gap-3 rounded-xl border border-amber-500/25 bg-amber-500/10 p-3">
                <TriangleAlert className="mt-0.5 h-4 w-4 text-amber-400" />
                <div>
                  <p className="text-sm font-medium">
                    {manual} local item{manual === 1 ? "" : "s"} not included
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Untracked or manually added files must be shared separately.
                  </p>
                </div>
              </div>
            )}
          </div>
          <Button
            variant="primary"
            className="w-full"
            onClick={copyCode}
            loading={busy}
          >
            <Clipboard className="h-4 w-4" />
            Copy compact share code
          </Button>
          <p className="flex items-center justify-center gap-1.5 text-center text-[11px] text-muted-foreground">
            <Link2 className="h-3 w-3" />
            No account or hosted backend required.
          </p>
        </div>
      )}
    </Modal>
  );
}
function Count({ value, label }: { value: number; label: string }) {
  return (
    <div className="rounded-lg bg-background/50 p-2">
      <p className="font-bold">{value}</p>
      <p className="text-[11px] text-muted-foreground">{label}</p>
    </div>
  );
}

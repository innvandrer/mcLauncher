import { useEffect, useState } from "react";
import { Upload, Link, RefreshCw } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, Field, Input, Modal } from "./ui";
import { api, errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";
import { cn } from "@/lib/utils";

type Variant = "classic" | "slim";

interface Props {
  open: boolean;
  onClose: () => void;
  accountId: string;
}

export function SkinModal({ open, onClose, accountId }: Props) {
  const toast = useStore((s) => s.toast);

  const [currentSkin, setCurrentSkin] = useState<{ url: string; variant: string } | null>(null);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<"url" | "file">("url");
  const [url, setUrl] = useState("");
  const [filePath, setFilePath] = useState("");
  const [variant, setVariant] = useState<Variant>("classic");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setCurrentSkin(null);
    setUrl("");
    setFilePath("");
    api
      .getSkin()
      .then((s) => {
        setCurrentSkin(s);
        setVariant((s.variant === "slim" ? "slim" : "classic") as Variant);
      })
      .catch(() => {}) // silently ignore (offline account etc.)
      .finally(() => setLoading(false));
  }, [open]);

  const pickFile = async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "PNG skin", extensions: ["png"] }],
    });
    if (typeof selected === "string") setFilePath(selected);
  };

  const save = async () => {
    setSaving(true);
    try {
      if (tab === "url") {
        if (!url.trim()) { toast("error", "Enter a skin URL"); return; }
        await api.setSkinUrl(url.trim(), variant);
      } else {
        if (!filePath) { toast("error", "Choose a PNG file"); return; }
        await api.setSkinFile(filePath, variant);
      }
      toast("success", "Skin updated!");
      // Refresh preview
      const s = await api.getSkin();
      setCurrentSkin(s);
      onClose();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Change Skin"
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={save} loading={saving}>
            Apply skin
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        {/* Current skin preview */}
        <div className="flex items-center gap-4">
          <div className="flex h-24 w-20 items-center justify-center rounded-xl border bg-muted/40">
            {loading ? (
              <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
            ) : currentSkin ? (
              <img
                src={`https://mc-heads.net/body/${encodeURIComponent(accountId)}/80`}
                alt="Current skin"
                className="h-full w-full rounded-xl object-contain [image-rendering:pixelated]"
              />
            ) : (
              <span className="text-xs text-muted-foreground">No preview</span>
            )}
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">Current skin</p>
            <p className="mt-0.5 truncate text-xs text-muted-foreground">
              {loading
                ? "Loading…"
                : currentSkin
                ? `Variant: ${currentSkin.variant}`
                : "Could not load skin"}
            </p>
            {currentSkin && (
              <a
                href={currentSkin.url}
                target="_blank"
                rel="noreferrer"
                className="mt-1 text-xs text-accent underline-offset-2 hover:underline"
              >
                View PNG
              </a>
            )}
          </div>
        </div>

        {/* Model / variant */}
        <Field label="Model">
          <div className="flex gap-2">
            {(["classic", "slim"] as Variant[]).map((v) => (
              <button
                key={v}
                onClick={() => setVariant(v)}
                className={cn(
                  "flex-1 rounded-lg border px-3 py-2 text-sm font-medium capitalize transition",
                  variant === v
                    ? "border-accent bg-accent/15 text-foreground"
                    : "border-border bg-muted/40 text-muted-foreground hover:bg-muted",
                )}
              >
                {v}
              </button>
            ))}
          </div>
        </Field>

        {/* Tab: URL or file */}
        <div className="flex gap-1 rounded-lg border bg-muted/30 p-1">
          {(["url", "file"] as const).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition",
                tab === t
                  ? "bg-background shadow text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {t === "url" ? <Link className="h-3.5 w-3.5" /> : <Upload className="h-3.5 w-3.5" />}
              {t === "url" ? "From URL" : "Upload file"}
            </button>
          ))}
        </div>

        {tab === "url" ? (
          <Field label="Skin URL" hint="Direct link to a 64×64 PNG skin image.">
            <Input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://textures.minecraft.net/texture/…"
            />
          </Field>
        ) : (
          <Field label="Skin file" hint="Select a 64×64 PNG from your computer.">
            <div className="flex gap-2">
              <Input
                value={filePath}
                readOnly
                placeholder="No file selected"
                className="flex-1"
              />
              <Button variant="secondary" onClick={pickFile}>
                Browse
              </Button>
            </div>
          </Field>
        )}
      </div>
    </Modal>
  );
}

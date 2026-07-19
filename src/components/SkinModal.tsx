import { useEffect, useState } from "react";
import {
  Upload,
  Link,
  RefreshCw,
  Save,
  Trash2,
  Plus,
  Search,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, Field, Input, Modal } from "./ui";
import { api, errMessage } from "@/lib/api";
import { useStore } from "@/store/useStore";
import { cn } from "@/lib/utils";
import type { PlayerSkin, SavedSkin } from "@/lib/types";

/** Crops the 8×8 face out of a 64×64 Minecraft skin texture as a small avatar. */
function SkinFace({ url, size = 44 }: { url: string; size?: number }) {
  const scale = size / 8;
  return (
    <div
      className="rounded-md border bg-muted/40"
      style={{
        width: size,
        height: size,
        backgroundImage: `url("${url}")`,
        backgroundSize: `${64 * scale}px ${64 * scale}px`,
        backgroundPosition: `-${8 * scale}px -${8 * scale}px`,
        imageRendering: "pixelated",
      }}
    />
  );
}

type Variant = "classic" | "slim";

interface Props {
  open: boolean;
  onClose: () => void;
  accountId: string;
}

export function SkinModal({ open, onClose, accountId }: Props) {
  const toast = useStore((s) => s.toast);

  const [currentSkin, setCurrentSkin] = useState<{
    url: string;
    variant: string;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<"url" | "file" | "player">("url");
  const [url, setUrl] = useState("");
  const [filePath, setFilePath] = useState("");
  const [variant, setVariant] = useState<Variant>("classic");
  const [saving, setSaving] = useState(false);
  const [wardrobe, setWardrobe] = useState<SavedSkin[]>([]);
  const [playerQuery, setPlayerQuery] = useState("");
  const [playerSkin, setPlayerSkin] = useState<PlayerSkin | null>(null);
  const [findingPlayer, setFindingPlayer] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    setCurrentSkin(null);
    setUrl("");
    setFilePath("");
    setPlayerQuery("");
    setPlayerSkin(null);
    api
      .getSkin()
      .then((s) => {
        setCurrentSkin(s);
        setVariant((s.variant === "slim" ? "slim" : "classic") as Variant);
      })
      .catch(() => {}) // silently ignore (offline account etc.)
      .finally(() => setLoading(false));
    api
      .listSavedSkins()
      .then(setWardrobe)
      .catch(() => {});
  }, [open]);

  const saveCurrent = async () => {
    if (!currentSkin) return;
    const name = window.prompt("Name this skin", `Skin ${wardrobe.length + 1}`);
    if (name == null) return;
    try {
      const list = await api.saveSkin(
        name || `Skin ${wardrobe.length + 1}`,
        currentSkin.url,
        currentSkin.variant,
      );
      setWardrobe(list);
      toast("success", "Saved to wardrobe");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const applySaved = async (s: SavedSkin) => {
    setSaving(true);
    try {
      if (s.kind === "file" && s.path) {
        await api.setSkinFile(s.path, s.variant);
      } else {
        await api.setSkinUrl(s.url, s.variant);
      }
      toast("success", `Applied “${s.name}”`);
      const cur = await api.getSkin().catch(() => null);
      if (cur) setCurrentSkin(cur);
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const addFileToWardrobe = async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "PNG skin", extensions: ["png"] }],
    });
    if (typeof selected !== "string") return;
    const base = selected.replace(/^.*[\\/]/, "").replace(/\.png$/i, "");
    const fallback = base || `Skin ${wardrobe.length + 1}`;
    const name = window.prompt("Name this skin", fallback);
    if (name == null) return;
    try {
      const list = await api.saveSkinFile(
        name.trim() || fallback,
        selected,
        variant,
      );
      setWardrobe(list);
      toast("success", "Added to wardrobe");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const removeSaved = async (id: string) => {
    try {
      setWardrobe(await api.deleteSavedSkin(id));
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const pickFile = async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "PNG skin", extensions: ["png"] }],
    });
    if (typeof selected === "string") setFilePath(selected);
  };

  const findPlayer = async () => {
    const q = playerQuery.trim();
    if (!q) return;
    setFindingPlayer(true);
    setPlayerSkin(null);
    try {
      const s = await api.fetchPlayerSkin(q);
      setPlayerSkin(s);
      setVariant(s.variant === "slim" ? "slim" : "classic");
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setFindingPlayer(false);
    }
  };

  const savePlayerSkin = async () => {
    if (!playerSkin) return;
    try {
      const list = await api.saveSkin(
        playerSkin.username,
        playerSkin.url,
        playerSkin.variant,
      );
      setWardrobe(list);
      toast("success", "Saved to wardrobe");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const save = async () => {
    setSaving(true);
    try {
      if (tab === "url") {
        if (!url.trim()) {
          toast("error", "Enter a skin URL");
          return;
        }
        await api.setSkinUrl(url.trim(), variant);
      } else if (tab === "file") {
        if (!filePath) {
          toast("error", "Choose a PNG file");
          return;
        }
        await api.setSkinFile(filePath, variant);
      } else {
        if (!playerSkin) {
          toast("error", "Find a player first");
          return;
        }
        await api.setSkinUrl(playerSkin.url, playerSkin.variant);
      }
      toast("success", "Skin updated!");
      // Refresh preview (tolerate offline accounts).
      const s = await api.getSkin().catch(() => null);
      if (s) setCurrentSkin(s);
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
              <div className="mt-1 flex items-center gap-3">
                <a
                  href={currentSkin.url}
                  target="_blank"
                  rel="noreferrer"
                  className="text-xs text-accent underline-offset-2 hover:underline"
                >
                  View PNG
                </a>
                <button
                  onClick={saveCurrent}
                  className="inline-flex items-center gap-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus"
                >
                  <Save className="h-3 w-3" /> Save to wardrobe
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Wardrobe — saved skins, one-click apply (no URL needed) */}
        <div>
          <div className="mb-2 flex items-center justify-between">
            <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Wardrobe ({wardrobe.length})
            </p>
            <button
              onClick={addFileToWardrobe}
              className="inline-flex items-center gap-1 text-xs text-accent transition hover:brightness-110 btn-focus"
            >
              <Plus className="h-3.5 w-3.5" /> Add PNG
            </button>
          </div>
          {wardrobe.length === 0 ? (
            <p className="rounded-lg border border-dashed p-3 text-xs text-muted-foreground">
              No saved skins yet. Add a PNG from your computer, or save your
              current skin — then switch with one click, no URL needed.
            </p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {wardrobe.map((s) => (
                <div key={s.id} className="group relative">
                  <button
                    onClick={() => applySaved(s)}
                    disabled={saving}
                    title={`Apply “${s.name}” (${s.variant})`}
                    className="block rounded-md ring-offset-2 ring-offset-background transition hover:ring-2 hover:ring-accent disabled:opacity-50 btn-focus"
                  >
                    <SkinFace
                      url={s.kind === "file" ? (s.image ?? "") : s.url}
                    />
                  </button>
                  <button
                    onClick={() => removeSaved(s.id)}
                    title="Remove from wardrobe"
                    className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-card text-muted-foreground opacity-0 shadow transition hover:text-destructive group-hover:opacity-100"
                  >
                    <Trash2 className="h-2.5 w-2.5" />
                  </button>
                  <p
                    className="mt-1 max-w-[44px] truncate text-center text-[10px] text-muted-foreground"
                    title={s.name}
                  >
                    {s.name}
                  </p>
                </div>
              ))}
            </div>
          )}
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

        {/* Tab: player search, URL, or file */}
        <div className="flex gap-1 rounded-lg border bg-muted/30 p-1">
          {(
            [
              { id: "player", label: "Find player", icon: Search },
              { id: "url", label: "From URL", icon: Link },
              { id: "file", label: "Upload file", icon: Upload },
            ] as const
          ).map((t) => {
            const Icon = t.icon;
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={cn(
                  "flex flex-1 items-center justify-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition",
                  tab === t.id
                    ? "bg-background shadow text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <Icon className="h-3.5 w-3.5" />
                {t.label}
              </button>
            );
          })}
        </div>

        {tab === "player" && (
          <div className="space-y-3">
            <Field
              label="Player name or NameMC link"
              hint="Grab any player's skin — type a username or UUID, or paste a namemc.com/profile link. Click Apply skin below."
            >
              <div className="flex gap-2">
                <Input
                  value={playerQuery}
                  onChange={(e) => setPlayerQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && findPlayer()}
                  placeholder="e.g. Notch   or   namemc.com/profile/…"
                />
                <Button
                  variant="secondary"
                  onClick={findPlayer}
                  loading={findingPlayer}
                >
                  <Search className="h-4 w-4" /> Find
                </Button>
              </div>
            </Field>
            {playerSkin && (
              <div className="flex items-center gap-3 rounded-lg border bg-card/60 p-3">
                <img
                  src={`https://mc-heads.net/body/${encodeURIComponent(playerSkin.uuid)}/72`}
                  alt=""
                  className="h-20 [image-rendering:pixelated]"
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">
                    {playerSkin.username}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Model: {playerSkin.variant}
                  </p>
                  <button
                    onClick={savePlayerSkin}
                    className="mt-1.5 inline-flex items-center gap-1 text-xs text-accent transition hover:brightness-110 btn-focus"
                  >
                    <Save className="h-3 w-3" /> Save to wardrobe
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {tab === "url" && (
          <Field label="Skin URL" hint="Direct link to a 64×64 PNG skin image.">
            <Input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://textures.minecraft.net/texture/…"
            />
          </Field>
        )}

        {tab === "file" && (
          <Field
            label="Skin file"
            hint="Select a 64×64 PNG from your computer."
          >
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

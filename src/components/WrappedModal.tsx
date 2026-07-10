import { useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { Copy, Download } from "lucide-react";
import { Button, Modal } from "./ui";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import type { Session } from "@/lib/types";

/**
 * "EZMapa Wrapped" — a year-in-review card computed from the local play
 * sessions and rendered onto a canvas, so it can be copied or saved as a
 * shareable PNG. No data leaves the machine unless the user shares the image.
 */
export function WrappedModal({
  open,
  onClose,
  sessions,
}: {
  open: boolean;
  onClose: () => void;
  sessions: Session[];
}) {
  const instances = useStore((s) => s.instances);
  const accounts = useStore((s) => s.accounts);
  const toast = useStore((s) => s.toast);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [screenshotCount, setScreenshotCount] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);

  const year = new Date().getFullYear();
  const username = accounts.find((a) => a.active)?.username ?? "Player";

  // Count screenshots across all instances once per open (cheap local reads).
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    Promise.all(instances.map((i) => api.listScreenshots(i.id).catch(() => [])))
      .then((lists) => {
        if (!cancelled) setScreenshotCount(lists.reduce((a, l) => a + l.length, 0));
      })
      .catch(() => !cancelled && setScreenshotCount(0));
    return () => {
      cancelled = true;
    };
  }, [open, instances]);

  const stats = useMemo(() => {
    const yearSessions = sessions.filter(
      (s) => new Date(s.started * 1000).getFullYear() === year,
    );
    const totalSeconds = yearSessions.reduce((a, s) => a + s.seconds, 0);
    const dayKeys = new Set(
      yearSessions.map((s) => {
        const d = new Date(s.started * 1000);
        return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
      }),
    );
    const longest = yearSessions.reduce<Session | null>(
      (best, s) => (best === null || s.seconds > best.seconds ? s : best),
      null,
    );
    const byInstance = new Map<string, number>();
    for (const s of yearSessions) {
      byInstance.set(s.instanceId, (byInstance.get(s.instanceId) ?? 0) + s.seconds);
    }
    let topId: string | null = null;
    let topSeconds = 0;
    for (const [id, secs] of byInstance) {
      if (secs > topSeconds) {
        topId = id;
        topSeconds = secs;
      }
    }
    return {
      totalSeconds,
      sessionCount: yearSessions.length,
      daysPlayed: dayKeys.size,
      longest,
      topName: (topId && instances.find((i) => i.id === topId)?.name) || "a former instance",
      topSeconds,
      modCount: instances.reduce((a, i) => a + (i.modCount || 0), 0),
      instanceCount: instances.length,
    };
  }, [sessions, instances, year]);

  const empty = stats.sessionCount === 0;

  useEffect(() => {
    if (!open || empty || screenshotCount === null) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    drawCard(canvas, {
      year,
      username,
      totalSeconds: stats.totalSeconds,
      sessionCount: stats.sessionCount,
      daysPlayed: stats.daysPlayed,
      longestSeconds: stats.longest?.seconds ?? 0,
      longestDate: stats.longest
        ? new Date(stats.longest.started * 1000).toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
          })
        : "",
      topName: stats.topName,
      topSeconds: stats.topSeconds,
      modCount: stats.modCount,
      instanceCount: stats.instanceCount,
      screenshotCount,
    });
  }, [open, empty, screenshotCount, stats, username, year]);

  const toBlob = () =>
    new Promise<Blob | null>((resolve) => canvasRef.current?.toBlob(resolve, "image/png"));

  const copyImage = async () => {
    try {
      const blob = await toBlob();
      if (!blob) throw new Error("Could not render the card.");
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      toast("success", "Card copied — paste it anywhere");
    } catch (e) {
      toast("error", errMessage(e));
    }
  };

  const savePng = async () => {
    setSaving(true);
    try {
      const dest = await save({
        defaultPath: `EZMapa-Wrapped-${year}.png`,
        filters: [{ name: "PNG image", extensions: ["png"] }],
      });
      if (!dest) return;
      const blob = await toBlob();
      if (!blob) throw new Error("Could not render the card.");
      await api.savePng(dest, Array.from(new Uint8Array(await blob.arrayBuffer())));
      toast("success", "Wrapped card saved");
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
      title={`EZMapa Wrapped ${year}`}
      size="lg"
      footer={
        !empty ? (
          <>
            <Button variant="ghost" onClick={copyImage}>
              <Copy className="h-4 w-4" /> Copy image
            </Button>
            <Button variant="primary" onClick={savePng} loading={saving}>
              <Download className="h-4 w-4" /> Save PNG
            </Button>
          </>
        ) : undefined
      }
    >
      {empty ? (
        <p className="py-8 text-center text-sm text-muted-foreground">
          No play sessions recorded in {year} yet. Launch an instance, play a
          bit, and come back for your Wrapped!
        </p>
      ) : (
        <canvas
          ref={canvasRef}
          className="w-full rounded-xl border border-white/10"
          aria-label={`EZMapa Wrapped ${year} stats card`}
        />
      )}
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Canvas rendering
// ---------------------------------------------------------------------------

interface CardData {
  year: number;
  username: string;
  totalSeconds: number;
  sessionCount: number;
  daysPlayed: number;
  longestSeconds: number;
  longestDate: string;
  topName: string;
  topSeconds: number;
  modCount: number;
  instanceCount: number;
  screenshotCount: number;
}

const W = 1200;
const H = 675;
const SCALE = 2; // export at 2400×1350 so the card stays crisp when shared

function hoursLabel(seconds: number): string {
  const h = seconds / 3600;
  if (h >= 10) return `${Math.round(h)} hours`;
  if (h >= 1) return `${Math.round(h * 10) / 10} hours`;
  return `${Math.max(1, Math.round(seconds / 60))} minutes`;
}

function roundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function ellipsize(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
  if (ctx.measureText(text).width <= maxWidth) return text;
  let cut = text;
  while (cut.length > 1 && ctx.measureText(`${cut}…`).width > maxWidth) {
    cut = cut.slice(0, -1);
  }
  return `${cut}…`;
}

const FONT = "system-ui, -apple-system, 'Segoe UI', sans-serif";

function drawCard(canvas: HTMLCanvasElement, d: CardData) {
  canvas.width = W * SCALE;
  canvas.height = H * SCALE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.scale(SCALE, SCALE);

  // The user's accent color drives the glow; the card itself is always dark.
  const accent =
    getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() ||
    "262 83% 58%";

  ctx.fillStyle = "#0c0e13";
  ctx.fillRect(0, 0, W, H);

  let glow = ctx.createRadialGradient(0, 0, 0, 0, 0, W * 0.75);
  glow.addColorStop(0, `hsl(${accent} / 0.35)`);
  glow.addColorStop(1, "transparent");
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, W, H);
  glow = ctx.createRadialGradient(W, H, 0, W, H, W * 0.65);
  glow.addColorStop(0, `hsl(${accent} / 0.18)`);
  glow.addColorStop(1, "transparent");
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, W, H);

  const P = 64;

  // Header
  ctx.fillStyle = `hsl(${accent})`;
  ctx.font = `700 20px ${FONT}`;
  ctx.fillText("E Z M A P A   W R A P P E D", P, P + 16);
  ctx.fillStyle = "#ffffff";
  ctx.font = `800 104px ${FONT}`;
  ctx.fillText(String(d.year), P, P + 116);
  ctx.font = `600 22px ${FONT}`;
  ctx.fillStyle = "rgba(255,255,255,0.65)";
  ctx.textAlign = "right";
  ctx.fillText(ellipsize(ctx, d.username, 320), W - P, P + 20);
  ctx.textAlign = "left";

  // Hero stat
  ctx.fillStyle = "#ffffff";
  ctx.font = `800 54px ${FONT}`;
  ctx.fillText(`${hoursLabel(d.totalSeconds)} played`, P, 288);
  ctx.font = `500 22px ${FONT}`;
  ctx.fillStyle = "rgba(255,255,255,0.65)";
  ctx.fillText(
    `across ${d.sessionCount.toLocaleString()} sessions on ${d.daysPlayed.toLocaleString()} different days`,
    P,
    324,
  );

  // Stat tiles (3 × 2)
  const tiles: { label: string; value: string }[] = [
    { label: "Most played", value: d.topName },
    { label: "Time in it", value: hoursLabel(d.topSeconds) },
    {
      label: "Longest session",
      value: d.longestSeconds > 0 ? `${hoursLabel(d.longestSeconds)} · ${d.longestDate}` : "—",
    },
    { label: "Instances", value: d.instanceCount.toLocaleString() },
    { label: "Mods installed", value: d.modCount.toLocaleString() },
    { label: "Screenshots", value: d.screenshotCount.toLocaleString() },
  ];
  const cols = 3;
  const gap = 20;
  const tw = (W - P * 2 - gap * (cols - 1)) / cols;
  const th = 104;
  const ty0 = 372;
  tiles.forEach((t, i) => {
    const x = P + (i % cols) * (tw + gap);
    const y = ty0 + Math.floor(i / cols) * (th + gap);
    roundedRect(ctx, x, y, tw, th, 16);
    ctx.fillStyle = "rgba(255,255,255,0.05)";
    ctx.fill();
    ctx.strokeStyle = "rgba(255,255,255,0.10)";
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.fillStyle = "rgba(255,255,255,0.55)";
    ctx.font = `600 14px ${FONT}`;
    ctx.fillText(t.label.toUpperCase(), x + 20, y + 34);
    ctx.fillStyle = "#ffffff";
    ctx.font = `700 27px ${FONT}`;
    ctx.fillText(ellipsize(ctx, t.value, tw - 40), x + 20, y + 74);
  });

  // Footer
  ctx.fillStyle = `hsl(${accent})`;
  ctx.beginPath();
  ctx.arc(P + 5, H - 46, 5, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = "rgba(255,255,255,0.55)";
  ctx.font = `500 17px ${FONT}`;
  ctx.fillText("Made with EZMapa — the friendly Minecraft launcher", P + 20, H - 40);
}

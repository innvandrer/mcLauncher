import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Download } from "lucide-react";
import { useStore } from "@/store/useStore";

/**
 * Full-window drop target: drag an EZMapa instance `.zip` or a Modrinth
 * modpack `.mrpack` onto the launcher to import it. Listens to Tauri's native
 * drag-drop events so it works with files dragged from the OS file manager.
 */
export function DropZone() {
  const [over, setOver] = useState(false);
  const importFromPath = useStore((s) => s.importInstanceFromPath);
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "over" || p.type === "enter") {
          setOver(true);
        } else if (p.type === "leave") {
          setOver(false);
        } else if (p.type === "drop") {
          setOver(false);
          const files = p.paths.filter((path) => {
            const lower = path.toLowerCase();
            return lower.endsWith(".zip") || lower.endsWith(".mrpack");
          });
          if (files.length === 0) {
            toast("error", "Drop an instance .zip or a .mrpack modpack to import it.");
            return;
          }
          // Import sequentially so multiple drops don't race the refresh.
          files.reduce(
            (chain, path) => chain.then(() => importFromPath(path)),
            Promise.resolve(),
          );
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* drag-drop unavailable (e.g. running in a browser dev preview) */
      });
    return () => unlisten?.();
  }, [importFromPath, toast]);

  return (
    <AnimatePresence>
      {over && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="pointer-events-none fixed inset-0 z-[90] flex items-center justify-center bg-black/60 backdrop-blur-sm"
        >
          <div className="flex flex-col items-center gap-3 rounded-2xl border-2 border-dashed border-accent bg-card/80 px-12 py-10 text-center">
            <Download className="h-10 w-10 text-accent" />
            <p className="text-lg font-semibold">Drop to import</p>
            <p className="text-sm text-muted-foreground">
              Release an instance <span className="font-mono">.zip</span> or a{" "}
              <span className="font-mono">.mrpack</span> modpack to add it.
            </p>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

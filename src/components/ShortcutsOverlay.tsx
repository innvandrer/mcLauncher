import { useEffect, useState } from "react";
import { Modal } from "@/components/ui";

const SHORTCUTS: { keys: string[]; label: string }[] = [
  { keys: ["Ctrl", "K"], label: "Open the command palette" },
  { keys: ["?"], label: "Show this shortcuts list" },
  { keys: ["Esc"], label: "Close a dialog / go back" },
  { keys: ["↑", "↓"], label: "Move through the command palette" },
  { keys: ["Enter"], label: "Run the highlighted command" },
];

/** A keyboard-shortcuts cheat sheet, toggled with the `?` key. */
export function ShortcutsOverlay() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing =
        !!t &&
        (t.tagName === "INPUT" ||
          t.tagName === "TEXTAREA" ||
          t.isContentEditable);
      if (e.key === "?" && !typing && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        setOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <Modal
      open={open}
      onClose={() => setOpen(false)}
      title="Keyboard shortcuts"
      size="sm"
    >
      <div className="space-y-2.5">
        {SHORTCUTS.map((s) => (
          <div
            key={s.label}
            className="flex items-center justify-between gap-4"
          >
            <span className="text-sm text-muted-foreground">{s.label}</span>
            <span className="flex shrink-0 items-center gap-1">
              {s.keys.map((k) => (
                <kbd
                  key={k}
                  className="rounded border bg-muted/60 px-1.5 py-0.5 text-[11px] font-medium text-foreground"
                >
                  {k}
                </kbd>
              ))}
            </span>
          </div>
        ))}
      </div>
    </Modal>
  );
}

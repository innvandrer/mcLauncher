import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function BeaconLogo({ className }: { className?: string }) {
  // Use the actual app icon (same artwork as the Windows taskbar / installer
  // icon) so the in-app brand matches the OS icon exactly.
  return (
    <img
      src="/beacon-icon.png"
      alt="Beacon"
      draggable={false}
      className={className}
    />
  );
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-10 shrink-0 select-none items-center justify-between border-b border-border/60 bg-surface/40 pl-3 glass"
    >
      <div data-tauri-drag-region className="flex items-center gap-2">
        <BeaconLogo className="h-5 w-5" />
        <span data-tauri-drag-region className="text-sm font-semibold tracking-tight">
          Beacon
        </span>
      </div>

      <div className="flex items-center">
        <WinButton onClick={() => appWindow.minimize()} aria-label="Minimize">
          <Minus className="h-4 w-4" />
        </WinButton>
        <WinButton onClick={() => appWindow.toggleMaximize()} aria-label="Maximize">
          <Square className="h-3 w-3" />
        </WinButton>
        <WinButton onClick={() => appWindow.close()} aria-label="Close" danger>
          <X className="h-4 w-4" />
        </WinButton>
      </div>
    </div>
  );
}

function WinButton({
  children,
  onClick,
  danger,
  ...rest
}: {
  children: React.ReactNode;
  onClick: () => void;
  danger?: boolean;
  "aria-label": string;
}) {
  return (
    <button
      onClick={onClick}
      {...rest}
      className={`flex h-10 w-12 items-center justify-center text-muted-foreground transition-colors ${
        danger ? "hover:bg-destructive hover:text-white" : "hover:bg-muted hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}

import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const appWindow = getCurrentWindow();

export function BeaconLogo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} fill="none" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="beam" x1="16" y1="2" x2="16" y2="30" gradientUnits="userSpaceOnUse">
          <stop stopColor="hsl(var(--accent))" stopOpacity="0.95" />
          <stop offset="1" stopColor="hsl(var(--accent))" stopOpacity="0.25" />
        </linearGradient>
      </defs>
      <path d="M16 3 L22 16 L10 16 Z" fill="url(#beam)" />
      <rect x="10" y="18" width="12" height="11" rx="2.5" fill="hsl(var(--accent))" />
      <rect x="13.5" y="21.5" width="5" height="5" rx="1.2" fill="hsl(var(--accent-foreground))" fillOpacity="0.85" />
    </svg>
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

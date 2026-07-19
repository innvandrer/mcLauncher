import type { Loader } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * Recognisable, offline SVG marks for each mod loader. Drawn on a transparent
 * background so the surrounding themed gradient shows through the padding.
 */
export function LoaderLogo({
  loader,
  className,
}: {
  loader: Loader;
  className?: string;
}) {
  switch (loader) {
    case "fabric":
      return (
        <img
          src="/fabric-logo.png"
          alt="Fabric"
          className={cn("object-contain", className)}
          style={{ imageRendering: "pixelated" }}
        />
      );
    case "quilt":
      return (
        <svg viewBox="0 0 32 32" className={className} aria-hidden>
          {/* Patchwork of pink/purple quilt squares */}
          <rect x="7" y="7" width="8" height="8" rx="1.5" fill="#e35aa8" />
          <rect x="17" y="7" width="8" height="8" rx="1.5" fill="#9a4bd6" />
          <rect x="7" y="17" width="8" height="8" rx="1.5" fill="#9a4bd6" />
          <rect x="17" y="17" width="8" height="8" rx="1.5" fill="#e35aa8" />
        </svg>
      );
    case "forge":
      return (
        <svg viewBox="0 0 32 32" className={className} aria-hidden>
          {/* Anvil silhouette */}
          <g fill="#cbd2dd">
            <polygon points="6,11 26,11 23.5,15.5 8.5,15.5" />
            <polygon points="26,11 29,12.2 26,13.6" />
            <rect x="13" y="15" width="6" height="5.5" />
            <rect x="9" y="20" width="14" height="3" rx="1" />
          </g>
        </svg>
      );
    case "neoforge":
      return (
        <svg viewBox="0 0 32 32" className={className} aria-hidden>
          {/* Flame in NeoForge orange */}
          <path
            d="M16 6c2.8 3.6 6 5.8 6 10a6 6 0 0 1-12 0c0-2 .8-3.3 2-4.4 0 1.2 1 2.1 2 2.1-1.2-3 .6-6.4 2-7.7z"
            fill="#f08a2c"
          />
          <path
            d="M16 13.5c1.4 1.6 2.6 2.7 2.6 4.6a2.6 2.6 0 0 1-5.2 0c0-1.3 1-2.3 1.6-3.1.2.7.6 1 1 1z"
            fill="#fcd9a8"
          />
        </svg>
      );
    case "vanilla":
    default:
      return (
        <svg viewBox="0 0 32 32" className={className} aria-hidden>
          {/* Grass block */}
          <rect x="7" y="7" width="18" height="18" rx="2.5" fill="#7c5435" />
          <path
            d="M7 9.5A2.5 2.5 0 0 1 9.5 7h13A2.5 2.5 0 0 1 25 9.5V14H7z"
            fill="#6bbb3b"
          />
          <rect
            x="11"
            y="17"
            width="2.5"
            height="2.5"
            rx="0.5"
            fill="#5b3d22"
          />
          <rect
            x="17.5"
            y="20"
            width="2.5"
            height="2.5"
            rx="0.5"
            fill="#5b3d22"
          />
          <rect x="19.5" y="16" width="2" height="2" rx="0.5" fill="#5b3d22" />
        </svg>
      );
  }
}

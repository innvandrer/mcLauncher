import { useEffect, useState } from "react";
import type { Instance } from "@/lib/types";
import { cn, isImageIcon } from "@/lib/utils";
import { LoaderLogo } from "./LoaderLogo";

// Themed gradient backing for the icon box, matched to each loader.
export const LOADER_ICON_BGS: Record<string, string> = {
  vanilla: "from-emerald-500/30 to-emerald-800/20",
  fabric: "from-amber-400/25 to-yellow-700/15",
  forge: "from-orange-500/30 to-red-600/20",
  neoforge: "from-orange-500/25 to-amber-700/15",
  quilt: "from-fuchsia-500/30 to-purple-600/20",
};

const SIZES = {
  sm: { box: "h-9 w-9 rounded-lg", emoji: "text-base", logo: "h-6 w-6" },
  card: { box: "h-14 w-14 rounded-xl", emoji: "text-2xl", logo: "h-10 w-10" },
  detail: {
    box: "h-16 w-16 rounded-2xl",
    emoji: "text-3xl",
    logo: "h-12 w-12",
  },
} as const;

function isATM10(instance: Instance): boolean {
  const n = instance.name.toLowerCase();
  return (
    n.includes("all the mods 10") || n.includes("atm10") || n.includes("atm 10")
  );
}

/**
 * The canonical way to render an instance's icon. Priority:
 *   1. A real image (modpack pack art, or ATM10's official icon).
 *   2. A user-picked emoji.
 *   3. The mod loader's logo on its themed background (the default).
 */
export function InstanceIcon({
  instance,
  size = "card",
  className,
}: {
  instance: Instance;
  size?: keyof typeof SIZES;
  className?: string;
}) {
  const [imgError, setImgError] = useState(false);
  // Reset the error flag whenever the icon source changes.
  useEffect(() => setImgError(false), [instance.id, instance.icon]);

  const s = SIZES[size];
  const atm = isATM10(instance);
  // Real pack art (set when a modpack is installed) takes priority; ATM10 keeps
  // its signature gold backing.
  const imageUrl = isImageIcon(instance.icon) ? instance.icon! : null;
  const bg = atm
    ? "from-yellow-400/40 to-yellow-700/20"
    : (LOADER_ICON_BGS[instance.loader] ?? LOADER_ICON_BGS.vanilla);

  // 1. Pack art / ATM10 image.
  if (imageUrl && !imgError) {
    return (
      <div
        className={cn(
          "overflow-hidden bg-gradient-to-br shadow-inner",
          s.box,
          bg,
          className,
        )}
      >
        <img
          src={imageUrl}
          alt={instance.name}
          className="h-full w-full object-cover"
          onError={() => setImgError(true)}
        />
      </div>
    );
  }

  // 2. User-picked emoji.
  if (instance.icon && !isImageIcon(instance.icon)) {
    return (
      <div
        className={cn(
          "flex items-center justify-center bg-gradient-to-br shadow-inner",
          s.box,
          bg,
          className,
        )}
      >
        <span className={s.emoji}>{instance.icon}</span>
      </div>
    );
  }

  // 3. Loader logo (the default).
  return (
    <div
      className={cn(
        "flex items-center justify-center bg-gradient-to-br shadow-inner",
        s.box,
        bg,
        className,
      )}
    >
      <LoaderLogo loader={instance.loader} className={s.logo} />
    </div>
  );
}

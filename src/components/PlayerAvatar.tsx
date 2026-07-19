import { useState } from "react";
import { UserCircle2 } from "lucide-react";
import { cn } from "@/lib/utils";

interface AvatarAccount {
  id: string;
  username: string;
  kind: string;
}

/**
 * Renders a Minecraft player head for an account using mc-heads.net. Microsoft
 * accounts use their real skin (via the profile UUID); offline accounts fall
 * back to the default Steve/Alex head for their generated UUID. If the image
 * fails to load (e.g. offline / network issue) we show the username initial.
 */
export function PlayerAvatar({
  account,
  size = 36,
  className,
}: {
  account?: AvatarAccount | null;
  size?: number;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  const dim = { width: size, height: size };

  if (!account) {
    return (
      <div
        className={cn(
          "flex items-center justify-center rounded-lg bg-accent/15 text-accent",
          className,
        )}
        style={dim}
      >
        <UserCircle2 style={{ width: size * 0.6, height: size * 0.6 }} />
      </div>
    );
  }

  if (failed) {
    return (
      <div
        className={cn(
          "flex items-center justify-center rounded-lg bg-accent/15 font-bold uppercase text-accent",
          className,
        )}
        style={dim}
      >
        {account.username.charAt(0)}
      </div>
    );
  }

  return (
    <img
      // 2× the rendered size keeps the pixel art crisp on high-DPI displays.
      src={`https://mc-heads.net/avatar/${encodeURIComponent(account.id)}/${size * 2}`}
      alt={account.username}
      onError={() => setFailed(true)}
      className={cn(
        "rounded-lg object-cover [image-rendering:pixelated]",
        className,
      )}
      style={dim}
    />
  );
}

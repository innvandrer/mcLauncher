import { useState } from "react";
import {
  Check,
  LogIn,
  Shirt,
  Trash2,
  UserCircle2,
  Users,
  UserPlus,
} from "lucide-react";
import { Button, Field, Input } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { cn } from "@/lib/utils";
import { PlayerAvatar } from "@/components/PlayerAvatar";
import { SkinModal } from "@/components/SkinModal";

export function AccountsPage() {
  const accounts = useStore((s) => s.accounts);
  const [skinAccountId, setSkinAccountId] = useState<string | null>(null);
  const loginMicrosoft = useStore((s) => s.loginMicrosoft);
  const addOffline = useStore((s) => s.addOffline);
  const setActive = useStore((s) => s.setActiveAccount);
  const remove = useStore((s) => s.removeAccount);
  const busy = useStore((s) => s.busy);
  const [offlineName, setOfflineName] = useState("");

  return (
    <>
      <div className="app-scroll">
        <div className="mx-auto max-w-2xl app-gutter py-6">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-accent/15 text-accent">
              <Users className="h-5 w-5" />
            </div>
            <div>
              <h1 className="text-2xl font-bold tracking-tight">Accounts</h1>
              <p className="text-sm text-muted-foreground">
                Sign in with Microsoft to play online, or add an offline account
                for testing.
              </p>
            </div>
          </div>

          <div className="mt-6 space-y-2">
            {accounts.length === 0 && (
              <div className="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
                No accounts yet.
              </div>
            )}
            {accounts.map((a) => (
              <div
                key={a.id}
                className={cn(
                  "flex items-center gap-3 rounded-xl border p-3 transition",
                  a.active ? "border-accent bg-accent/5" : "bg-card/60",
                )}
              >
                <AccountRender account={a} />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{a.username}</div>
                  <div className="text-xs text-muted-foreground">
                    {a.kind === "offline"
                      ? "Offline account"
                      : "Microsoft account"}
                  </div>
                </div>
                {a.active ? (
                  <span className="flex items-center gap-1 rounded-full bg-accent/15 px-2.5 py-1 text-xs font-medium text-accent">
                    <Check className="h-3.5 w-3.5" /> Active
                  </span>
                ) : (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setActive(a.id)}
                  >
                    Use
                  </Button>
                )}
                {a.kind === "microsoft" && (
                  <button
                    onClick={() => setSkinAccountId(a.id)}
                    title="Change skin"
                    className="rounded-md p-2 text-muted-foreground transition hover:bg-muted hover:text-accent btn-focus"
                  >
                    <Shirt className="h-4 w-4" />
                  </button>
                )}
                <button
                  onClick={() => remove(a.id)}
                  className="rounded-md p-2 text-muted-foreground transition hover:bg-muted hover:text-destructive btn-focus"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>

          <div className="mt-8 grid gap-4 sm:grid-cols-2">
            <div className="rounded-xl border bg-card/60 p-4">
              <div className="mb-1 flex items-center gap-2 font-medium">
                <LogIn className="h-4 w-4 text-accent" /> Microsoft
              </div>
              <p className="mb-3 text-sm text-muted-foreground">
                Sign in with your real Minecraft account to play multiplayer.
              </p>
              <Button
                variant="primary"
                className="w-full"
                loading={busy}
                onClick={loginMicrosoft}
              >
                {busy ? "Complete login in browser…" : "Add Microsoft account"}
              </Button>
            </div>

            <div className="rounded-xl border bg-card/60 p-4">
              <div className="mb-1 flex items-center gap-2 font-medium">
                <UserCircle2 className="h-4 w-4" /> Offline
              </div>
              <Field
                label=""
                hint="For singleplayer / testing without signing in."
              >
                <div className="flex gap-2">
                  <Input
                    value={offlineName}
                    onChange={(e) => setOfflineName(e.target.value)}
                    placeholder="Username"
                  />
                  <Button
                    variant="secondary"
                    onClick={() => {
                      if (offlineName.trim()) {
                        addOffline(offlineName.trim());
                        setOfflineName("");
                      }
                    }}
                  >
                    <UserPlus className="h-4 w-4" />
                  </Button>
                </div>
              </Field>
            </div>
          </div>
        </div>
      </div>
      <SkinModal
        open={skinAccountId !== null}
        onClose={() => setSkinAccountId(null)}
        accountId={skinAccountId ?? ""}
      />
    </>
  );
}

/** A full-body skin render for an account, falling back to the head avatar. */
function AccountRender({
  account,
}: {
  account: { id: string; username: string; kind: string };
}) {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return <PlayerAvatar account={account} size={48} className="shrink-0" />;
  }
  return (
    <img
      src={`https://mc-heads.net/body/${encodeURIComponent(account.id)}/64`}
      alt={account.username}
      onError={() => setFailed(true)}
      className="h-16 w-auto shrink-0 [image-rendering:pixelated]"
    />
  );
}

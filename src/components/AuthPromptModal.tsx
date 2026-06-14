import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, ExternalLink } from "lucide-react";
import { useState } from "react";
import { Button, Modal, Spinner } from "./ui";
import { useStore } from "@/store/useStore";

export function AuthPromptModal() {
  const prompt = useStore((s) => s.authPrompt);
  const dismiss = useStore((s) => s.dismissAuthPrompt);
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (!prompt) return;
    try {
      await navigator.clipboard.writeText(prompt.userCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* ignore */
    }
  };

  return (
    <Modal open={!!prompt} onClose={dismiss} title="Sign in with Microsoft" size="sm">
      {prompt && (
        <div className="space-y-4 text-center">
          <p className="text-sm text-muted-foreground">
            Open the page below and enter this code to sign in to your Microsoft account.
          </p>

          <button
            onClick={copy}
            className="group mx-auto flex items-center gap-3 rounded-xl border bg-muted/40 px-5 py-3 transition hover:bg-muted btn-focus"
          >
            <span className="font-mono text-2xl font-bold tracking-[0.3em]">
              {prompt.userCode}
            </span>
            <Copy className="h-4 w-4 text-muted-foreground group-hover:text-foreground" />
          </button>
          {copied && <p className="text-xs text-success">Copied to clipboard</p>}

          <Button
            variant="primary"
            className="w-full"
            onClick={() => openUrl(prompt.verificationUri)}
          >
            <ExternalLink className="h-4 w-4" />
            Open sign-in page
          </Button>

          <div className="flex items-center justify-center gap-2 pt-1 text-sm text-muted-foreground">
            <Spinner className="h-4 w-4" />
            Waiting for you to finish signing in…
          </div>
        </div>
      )}
    </Modal>
  );
}

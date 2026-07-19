import { Download } from "lucide-react";
import { Button, Modal } from "./ui";
import { useStore } from "@/store/useStore";

/**
 * Fallback when automatic startup update fails (network error, signature issue,
 * etc.). Successful updates install silently and relaunch before this is shown.
 */
export function UpdateModal() {
  const update = useStore((s) => s.update);
  const updating = useStore((s) => s.updating);
  const install = useStore((s) => s.installUpdate);
  const dismiss = useStore((s) => s.dismissUpdate);

  return (
    <Modal
      open={!!update}
      onClose={updating ? () => {} : dismiss}
      title="Update available"
      size="sm"
    >
      {update && (
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            EZMapa{" "}
            <span className="font-medium text-foreground">
              {update.version}
            </span>{" "}
            is available.{" "}
            {updating
              ? "Downloading and installing… the app will restart automatically."
              : "Install it now? The app will restart to finish."}
          </p>

          {update.notes && (
            <div className="max-h-40 overflow-y-auto whitespace-pre-wrap rounded-lg border bg-muted/40 p-3 text-xs text-muted-foreground">
              {update.notes}
            </div>
          )}

          <div className="flex gap-2">
            <Button
              variant="primary"
              className="flex-1"
              loading={updating}
              onClick={install}
            >
              <Download className="h-4 w-4" />
              Update &amp; restart
            </Button>
            {!updating && (
              <Button variant="secondary" onClick={dismiss}>
                Later
              </Button>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}

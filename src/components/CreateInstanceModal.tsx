import { useEffect, useMemo, useState } from "react";
import { FlaskConical, FolderOpen, Search, ShieldCheck } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Button, Field, Input, Modal, Select } from "./ui";
import { LoaderLogo } from "./LoaderLogo";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import { cn, LOADERS } from "@/lib/utils";
import type { Loader, LoaderVersion } from "@/lib/types";

// Empty string = "Auto": use the loader's logo as the icon.
const ICONS = [
  "🟩",
  "🔥",
  "⚙️",
  "🧪",
  "🏰",
  "🌲",
  "💎",
  "🚀",
  "🐉",
  "⛏️",
  "🧱",
  "✨",
];
export function CreateInstanceModal({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const versions = useStore((s) => s.versions);
  const createInstance = useStore((s) => s.createInstance);
  const importFromPath = useStore((s) => s.importInstanceFromPath);
  const toast = useStore((s) => s.toast);

  const [name, setName] = useState("");
  const [icon, setIcon] = useState(""); // "" = Auto (loader logo)
  const [mcVersion, setMcVersion] = useState("");
  const [showSnapshots, setShowSnapshots] = useState(false);
  const [filter, setFilter] = useState("");
  const [loader, setLoader] = useState<Loader>("vanilla");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [showBetaLoaders, setShowBetaLoaders] = useState(false);
  const [loadingLoaders, setLoadingLoaders] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  // Reset + sensible defaults each time the modal opens.
  useEffect(() => {
    if (open) {
      setName("");
      setIcon(""); // default to the loader logo
      setMcVersion(versions?.latestRelease ?? "");
      setShowSnapshots(false);
      setFilter("");
      setLoader("vanilla");
      setLoaderVersion("");
      setShowBetaLoaders(false);
    }
  }, [open, versions]);

  const filteredVersions = useMemo(() => {
    if (!versions) return [];
    return versions.versions
      .filter((v) => (showSnapshots ? true : v.kind === "release"))
      .filter((v) => v.id.toLowerCase().includes(filter.toLowerCase()))
      .slice(0, 300);
  }, [versions, showSnapshots, filter]);

  const visibleLoaderVersions = useMemo(
    () =>
      showBetaLoaders
        ? loaderVersions
        : loaderVersions.filter((version) => version.stable),
    [loaderVersions, showBetaLoaders],
  );

  const betaLoaderCount = useMemo(
    () => loaderVersions.filter((version) => !version.stable).length,
    [loaderVersions],
  );

  // Load loader versions when needed.
  useEffect(() => {
    let cancelled = false;
    if (!open || loader === "vanilla" || !mcVersion) {
      setLoaderVersions([]);
      setLoaderVersion("");
      return;
    }
    setLoadingLoaders(true);
    setLoaderVersions([]);
    setLoaderVersion("");
    setShowBetaLoaders(false);
    const fetchers: Record<string, (v: string) => Promise<LoaderVersion[]>> = {
      fabric: api.listFabric,
      quilt: api.listQuilt,
      forge: api.listForge,
      neoforge: api.listNeoforge,
    };
    const fetcher = fetchers[loader];
    if (!fetcher) {
      setLoadingLoaders(false);
      return;
    }
    fetcher(mcVersion)
      .then((list) => {
        if (cancelled) return;
        setLoaderVersions(list);
        const stable = list.find((l) => l.stable) ?? list[0];
        setLoaderVersion(stable?.version ?? "");
      })
      .catch((e) => !cancelled && toast("error", errMessage(e)))
      .finally(() => !cancelled && setLoadingLoaders(false));
    return () => {
      cancelled = true;
    };
  }, [open, loader, mcVersion, toast]);

  // If previews are hidden while one is selected, move back to the newest
  // stable build so an invisible beta can never be submitted accidentally.
  useEffect(() => {
    if (loadingLoaders || showBetaLoaders) return;
    const selected = loaderVersions.find((v) => v.version === loaderVersion);
    if (selected?.stable) return;
    setLoaderVersion(
      loaderVersions.find((version) => version.stable)?.version ?? "",
    );
  }, [loaderVersion, loaderVersions, loadingLoaders, showBetaLoaders]);

  const canSubmit =
    name.trim() && mcVersion && (loader === "vanilla" || loaderVersion);

  const toggleBetaLoaders = () => {
    const next = !showBetaLoaders;
    setShowBetaLoaders(next);
    if (next && !loaderVersion) {
      setLoaderVersion(loaderVersions[0]?.version ?? "");
    }
  };

  // Alternative to building an instance by hand: pick a .mrpack modpack or an
  // exported instance .zip and import it (same flow as dropping the file).
  const importFromFile = async () => {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "Modpack or instance", extensions: ["mrpack", "zip"] }],
    });
    if (typeof selected !== "string") return;
    onClose();
    await importFromPath(selected);
  };

  const submit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      await createInstance({
        name: name.trim(),
        mcVersion,
        loader,
        loaderVersion: loader === "vanilla" ? null : loaderVersion,
        icon: icon || null,
      });
      onClose();
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="New instance"
      size="lg"
      footer={
        <>
          <Button
            variant="ghost"
            className="mr-auto gap-2"
            onClick={importFromFile}
          >
            <FolderOpen className="h-4 w-4" />
            Import file…
          </Button>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={submit}
            loading={submitting}
            disabled={!canSubmit}
          >
            Create instance
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        {/* Name + icon */}
        <div className="flex gap-3">
          <div className="flex-1">
            <Field label="Name">
              <Input
                autoFocus
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My awesome pack"
              />
            </Field>
          </div>
        </div>
        <div>
          <span className="mb-1.5 block text-sm font-medium">Icon</span>
          <div className="flex flex-wrap gap-1.5">
            {/* Auto = the mod loader's own logo (the default). */}
            <button
              onClick={() => setIcon("")}
              title="Auto — use the loader logo"
              className={cn(
                "flex h-9 w-9 items-center justify-center rounded-lg transition btn-focus",
                icon === ""
                  ? "bg-accent/20 ring-2 ring-accent"
                  : "bg-muted/60 hover:bg-muted",
              )}
            >
              <LoaderLogo loader={loader} className="h-6 w-6" />
            </button>
            {ICONS.map((e) => (
              <button
                key={e}
                onClick={() => setIcon(e)}
                className={cn(
                  "flex h-9 w-9 items-center justify-center rounded-lg text-lg transition btn-focus",
                  icon === e
                    ? "bg-accent/20 ring-2 ring-accent"
                    : "bg-muted/60 hover:bg-muted",
                )}
              >
                {e}
              </button>
            ))}
          </div>
        </div>

        {/* Loader */}
        <div>
          <span className="mb-1.5 block text-sm font-medium">Mod loader</span>
          <div className="grid grid-cols-5 gap-1.5">
            {LOADERS.map((l) => {
              const active = loader === l.id;
              return (
                <button
                  key={l.id}
                  onClick={() => setLoader(l.id)}
                  className={cn(
                    "rounded-lg border px-2 py-2 text-sm font-medium transition btn-focus",
                    active
                      ? "border-accent bg-accent/15 text-foreground"
                      : "border-border bg-muted/40 text-muted-foreground hover:bg-muted",
                  )}
                >
                  {l.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Loader version */}
        {loader !== "vanilla" && (
          <div className="space-y-2.5">
            <Field
              label={`${LOADERS.find((l) => l.id === loader)?.label} version`}
            >
              <Select
                value={loaderVersion}
                onChange={(e) => setLoaderVersion(e.target.value)}
                disabled={loadingLoaders || visibleLoaderVersions.length === 0}
              >
                {loadingLoaders && <option>Loading…</option>}
                {!loadingLoaders && loaderVersions.length === 0 && (
                  <option>No versions for {mcVersion}</option>
                )}
                {!loadingLoaders &&
                  loaderVersions.length > 0 &&
                  visibleLoaderVersions.length === 0 && (
                    <option>No stable builds available</option>
                  )}
                {visibleLoaderVersions.map((version) => (
                  <option key={version.version} value={version.version}>
                    {version.version}
                    {version.stable ? "" : " — Beta"}
                  </option>
                ))}
              </Select>
            </Field>

            {!loadingLoaders && betaLoaderCount > 0 && (
              <button
                type="button"
                role="switch"
                aria-checked={showBetaLoaders}
                onClick={toggleBetaLoaders}
                className={cn(
                  "btn-focus flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-all",
                  showBetaLoaders
                    ? "border-warning/45 bg-warning/10 shadow-sm shadow-warning/10"
                    : "border-border bg-muted/25 hover:border-warning/30 hover:bg-muted/45",
                )}
              >
                <span
                  className={cn(
                    "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg transition-colors",
                    showBetaLoaders
                      ? "bg-warning/20 text-warning"
                      : "bg-muted text-muted-foreground",
                  )}
                >
                  {showBetaLoaders ? (
                    <FlaskConical className="h-4 w-4" />
                  ) : (
                    <ShieldCheck className="h-4 w-4" />
                  )}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium">
                    Include beta versions
                  </span>
                  <span className="block text-xs text-muted-foreground">
                    {showBetaLoaders
                      ? `${betaLoaderCount} experimental ${betaLoaderCount === 1 ? "build" : "builds"} available — may be unstable`
                      : "Off by default for a more reliable instance"}
                  </span>
                </span>
                <span
                  aria-hidden="true"
                  className={cn(
                    "relative h-6 w-11 shrink-0 rounded-full transition-colors",
                    showBetaLoaders ? "bg-warning" : "bg-muted-foreground/25",
                  )}
                >
                  <span
                    className={cn(
                      "absolute top-1 h-4 w-4 rounded-full bg-white shadow-sm transition-transform",
                      showBetaLoaders ? "translate-x-6" : "translate-x-1",
                    )}
                  />
                </span>
              </button>
            )}
          </div>
        )}

        {/* Minecraft version */}
        <div>
          <div className="mb-1.5 flex items-center justify-between">
            <span className="text-sm font-medium">Minecraft version</span>
            <label className="flex cursor-pointer items-center gap-2 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={showSnapshots}
                onChange={(e) => setShowSnapshots(e.target.checked)}
                className="accent-[hsl(var(--accent))]"
              />
              Show snapshots
            </label>
          </div>
          <div className="relative mb-2">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter versions…"
              className="pl-9"
            />
          </div>
          {!versions ? (
            <p className="rounded-lg bg-muted/40 p-3 text-sm text-muted-foreground">
              Couldn't load the version list (are you offline?). You can still
              type an exact version id above and it will be used.
            </p>
          ) : (
            <div className="scroll-area max-h-44 rounded-lg border bg-muted/20 p-1">
              {filteredVersions.map((v) => (
                <button
                  key={v.id}
                  onClick={() => setMcVersion(v.id)}
                  className={cn(
                    "flex w-full items-center justify-between rounded-md px-3 py-1.5 text-sm transition",
                    mcVersion === v.id
                      ? "bg-accent text-accent-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  <span>{v.id}</span>
                  {v.kind !== "release" && (
                    <span className="text-xs opacity-60">{v.kind}</span>
                  )}
                </button>
              ))}
            </div>
          )}
          {filter && !versions && (
            <button
              onClick={() => setMcVersion(filter.trim())}
              className="mt-2 text-sm text-accent hover:underline"
            >
              Use “{filter.trim()}”
            </button>
          )}
        </div>
      </div>
    </Modal>
  );
}

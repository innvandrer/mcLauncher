import { useEffect, useState } from "react";
import { ArrowLeft, Boxes, Download, ExternalLink, Search } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button, Input, Select, Spinner } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { api, errMessage } from "@/lib/api";
import { cn, formatNumber } from "@/lib/utils";
import type { ContentVersion, ModHit } from "@/lib/types";

type Provider = "modrinth" | "curseforge";

function projectUrl(hit: ModHit, provider: Provider): string {
  if (provider === "curseforge") {
    return `https://www.curseforge.com/minecraft/modpacks/${hit.slug}`;
  }
  return `https://modrinth.com/modpack/${hit.slug}`;
}

export function ModpacksPage() {
  const toast = useStore((s) => s.toast);

  const [provider, setProvider] = useState<Provider>("modrinth");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ModHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [selected, setSelected] = useState<ModHit | null>(null);

  useEffect(() => {
    if (selected) return; // Don't re-search while viewing detail
    setSearching(true);
    const t = setTimeout(() => {
      const search =
        provider === "curseforge"
          ? api.searchCurseforge({ query, contentType: "modpack", limit: 30 })
          : api.searchModrinth({ query, projectType: "modpack", limit: 30 });
      search
        .then((r) => setResults(r.hits))
        .catch((e) => toast("error", errMessage(e)))
        .finally(() => setSearching(false));
    }, 350);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, provider, selected]);

  if (selected) {
    return (
      <PackDetailView
        hit={selected}
        provider={provider}
        onBack={() => setSelected(null)}
      />
    );
  }

  return (
    <div className="app-page">
      <header className="app-gutter pt-6">
        <h1 className="text-2xl font-bold tracking-tight">Modpacks</h1>
        <p className="mt-0.5 text-sm text-muted-foreground">
          Browse and install complete modpacks. Each one becomes a new instance.
        </p>
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <div className="relative min-w-0 flex-1 basis-full sm:basis-auto">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search modpacks…"
              className="pl-9"
            />
          </div>
          <div className="inline-flex shrink-0 rounded-lg border bg-card/60 p-0.5">
            {(["modrinth", "curseforge"] as Provider[]).map((p) => (
              <button
                key={p}
                onClick={() => setProvider(p)}
                className={cn(
                  "rounded-md px-3 py-1.5 text-xs font-medium capitalize transition btn-focus",
                  provider === p
                    ? "bg-accent/20 text-accent"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {p}
              </button>
            ))}
          </div>
        </div>
      </header>

      <div className="app-scroll app-gutter py-5">
        {searching && results.length === 0 ? (
          <div className="flex justify-center py-16">
            <Spinner />
          </div>
        ) : results.length === 0 ? (
          <div className="py-16 text-center text-sm text-muted-foreground">
            No modpacks found
            {provider === "curseforge" && " (a CurseForge API key is required — see Settings)"}.
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 2xl:grid-cols-3">
            {results.map((hit) => (
              <button
                key={hit.project_id}
                onClick={() => setSelected(hit)}
                className="flex items-start gap-3 rounded-xl border bg-card/60 p-4 text-left transition hover:bg-card hover:border-accent/40 btn-focus"
              >
                {hit.icon_url ? (
                  <img
                    src={hit.icon_url}
                    alt=""
                    className="h-14 w-14 shrink-0 rounded-lg object-cover"
                  />
                ) : (
                  <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-lg bg-muted">
                    <Boxes className="h-6 w-6 text-muted-foreground" />
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <h3 className="truncate font-semibold">{hit.title}</h3>
                  <p className="text-xs text-muted-foreground">
                    {formatNumber(hit.downloads)} downloads
                    {hit.author && ` · ${hit.author}`}
                  </p>
                  <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">
                    {hit.description}
                  </p>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// --------------------------------------------------------------------------
// Full detail view — shown when a pack card is clicked
// --------------------------------------------------------------------------

function PackDetailView({
  hit,
  provider,
  onBack,
}: {
  hit: ModHit;
  provider: Provider;
  onBack: () => void;
}) {
  const toast = useStore((s) => s.toast);
  const refreshInstances = useStore((s) => s.refreshInstances);
  const setView = useStore((s) => s.setView);

  const [versions, setVersions] = useState<ContentVersion[]>([]);
  const [versionId, setVersionId] = useState("");
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [body, setBody] = useState("");
  const [loadingBody, setLoadingBody] = useState(false);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    // Fetch versions
    setLoadingVersions(true);
    const vf =
      provider === "curseforge"
        ? api.listCurseforgeFiles(hit.project_id)
        : api.listModrinthVersions(hit.project_id);
    vf.then((v) => {
      setVersions(v);
      setVersionId(v[0]?.id ?? "");
    })
      .catch((e) => toast("error", errMessage(e)))
      .finally(() => setLoadingVersions(false));

    // Fetch full description body (best-effort)
    setLoadingBody(true);
    setBody("");
    const bf =
      provider === "curseforge"
        ? api.getCurseforgeDescription(hit.project_id)
        : api.getModrinthProjectBody(hit.project_id);
    bf.then(setBody)
      .catch(() => {})
      .finally(() => setLoadingBody(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hit.project_id, provider]);

  const install = async () => {
    setInstalling(true);
    toast("info", `Installing "${hit.title}" — added to Instances`);
    // Jump to the Instances tab so the new instance shows its download
    // progress live while the content streams in.
    setView("instances");
    try {
      const inst =
        provider === "curseforge"
          ? await api.installCurseforgeModpack({
              projectId: hit.project_id,
              fileId: versionId || null,
              icon: hit.icon_url ?? null,
            })
          : await api.installModrinthModpack({
              projectId: hit.project_id,
              versionId: versionId || null,
              icon: hit.icon_url ?? null,
            });
      await refreshInstances();
      toast("success", `Installed "${inst.name}"`);
    } catch (e) {
      toast("error", errMessage(e));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <div className="app-page">
      {/* Sticky top bar */}
      <div className="app-gutter shrink-0 pt-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
          <button
            onClick={onBack}
            className="inline-flex items-center gap-1.5 text-sm text-muted-foreground transition hover:text-foreground btn-focus"
          >
            <ArrowLeft className="h-4 w-4" />
            Back to Modpacks
          </button>
          <button
            onClick={() => api.openUrl(projectUrl(hit, provider))}
            className="inline-flex items-center gap-1.5 text-sm text-muted-foreground transition hover:text-foreground btn-focus"
          >
            <ExternalLink className="h-3.5 w-3.5" />
            Open on {provider === "curseforge" ? "CurseForge" : "Modrinth"}
          </button>
        </div>

        {/* Pack header */}
        <div className="flex flex-wrap items-start gap-4">
          {hit.icon_url ? (
            <img
              src={hit.icon_url}
              alt=""
              className="h-20 w-20 shrink-0 rounded-2xl object-cover"
            />
          ) : (
            <div className="flex h-20 w-20 shrink-0 items-center justify-center rounded-2xl bg-muted">
              <Boxes className="h-8 w-8 text-muted-foreground" />
            </div>
          )}
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-2xl font-bold tracking-tight">{hit.title}</h1>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {hit.author && `by ${hit.author} · `}
              {formatNumber(hit.downloads)} downloads
            </p>
            <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">{hit.description}</p>
          </div>
        </div>

        {/* Version picker + install */}
        <div className="mt-4 flex flex-wrap items-center gap-3">
          {loadingVersions ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner className="h-4 w-4" />
              Loading versions…
            </div>
          ) : versions.length > 0 ? (
            <Select
              value={versionId}
              onChange={(e) => setVersionId(e.target.value)}
              className="max-w-sm"
            >
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                  {v.gameVersions.length > 0 && ` — MC ${v.gameVersions[0]}`}
                </option>
              ))}
            </Select>
          ) : null}
          <Button
            variant="primary"
            onClick={install}
            loading={installing}
            disabled={loadingVersions || !versionId}
          >
            <Download className="h-4 w-4" />
            Install
          </Button>
        </div>

        <hr className="mt-5 border-border" />
      </div>

      {/* Scrollable description */}
      <div className="app-scroll app-gutter py-5">
        {loadingBody ? (
          <div className="flex justify-center py-10">
            <Spinner />
          </div>
        ) : body ? (
          provider === "curseforge" ? (
            <div
              className="cf-body text-sm text-muted-foreground"
              dangerouslySetInnerHTML={{ __html: body }}
            />
          ) : (
            <div className="md-body">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ ...p }) => <h1 className="mb-3 mt-6 text-xl font-bold text-foreground first:mt-0" {...p} />,
                  h2: ({ ...p }) => <h2 className="mb-2 mt-5 text-lg font-semibold text-foreground" {...p} />,
                  h3: ({ ...p }) => <h3 className="mb-2 mt-4 text-base font-semibold text-foreground" {...p} />,
                  h4: ({ ...p }) => <h4 className="mb-1 mt-3 text-sm font-semibold text-foreground" {...p} />,
                  p: ({ ...p }) => <p className="mb-3 text-sm leading-relaxed text-muted-foreground" {...p} />,
                  ul: ({ ...p }) => <ul className="mb-3 ml-5 list-disc space-y-1 text-sm text-muted-foreground" {...p} />,
                  ol: ({ ...p }) => <ol className="mb-3 ml-5 list-decimal space-y-1 text-sm text-muted-foreground" {...p} />,
                  li: ({ ...p }) => <li className="text-sm text-muted-foreground" {...p} />,
                  a: ({ ...p }) => <a className="text-accent underline-offset-2 hover:underline" target="_blank" rel="noopener noreferrer" {...p} />,
                  img: ({ ...p }) => <img className="my-2 max-w-full rounded-lg" {...p} />,
                  blockquote: ({ ...p }) => <blockquote className="mb-3 border-l-4 border-accent/40 pl-4 italic text-sm text-muted-foreground" {...p} />,
                  hr: ({ ...p }) => <hr className="my-5 border-border" {...p} />,
                  strong: ({ ...p }) => <strong className="font-semibold text-foreground" {...p} />,
                  em: ({ ...p }) => <em className="italic" {...p} />,
                  code: ({ className, children, ...p }) => {
                    const isBlock = className?.includes("language-");
                    return isBlock ? (
                      <code className="block overflow-auto rounded-lg bg-muted p-3 font-mono text-xs text-foreground" {...p}>{children}</code>
                    ) : (
                      <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs text-foreground" {...p}>{children}</code>
                    );
                  },
                  pre: ({ ...p }) => <pre className="mb-3 overflow-auto rounded-lg bg-muted p-3 text-xs" {...p} />,
                  table: ({ ...p }) => <div className="mb-3 overflow-auto"><table className="w-full text-sm border-collapse" {...p} /></div>,
                  th: ({ ...p }) => <th className="border border-border p-2 text-left text-xs font-semibold text-foreground" {...p} />,
                  td: ({ ...p }) => <td className="border border-border p-2 text-xs text-muted-foreground" {...p} />,
                }}
              >
                {body}
              </ReactMarkdown>
            </div>
          )
        ) : (
          <p className="text-sm text-muted-foreground">No description available.</p>
        )}
      </div>
    </div>
  );
}

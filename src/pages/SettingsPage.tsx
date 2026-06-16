import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { Cpu, MonitorCog, Package, Palette, Search, Sparkles, Wand2 } from "lucide-react";
import { Button, Field, Input } from "@/components/ui";
import { useStore } from "@/store/useStore";
import { api } from "@/lib/api";
import { ACCENTS, AIKAR_FLAGS, cn } from "@/lib/utils";
import type { JavaInstall, Settings } from "@/lib/types";

/** Suggest a heap size: ~half of system RAM, clamped to a sane Minecraft range
 *  and snapped to the 512 MB slider step. */
function recommendedMemoryMb(totalMb: number): number {
  const half = totalMb * 0.5;
  const clamped = Math.min(8192, Math.max(2048, half));
  return Math.round(clamped / 512) * 512;
}

export function SettingsPage() {
  const settings = useStore((s) => s.settings);
  const saveSettings = useStore((s) => s.saveSettings);
  const [javaList, setJavaList] = useState<JavaInstall[] | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [totalRamMb, setTotalRamMb] = useState(0);
  const [version, setVersion] = useState("");

  useEffect(() => {
    api.systemMemoryMb().then(setTotalRamMb).catch(() => setTotalRamMb(0));
    getVersion().then(setVersion).catch(() => setVersion(""));
  }, []);

  if (!settings) return null;

  const recommended = totalRamMb > 0 ? recommendedMemoryMb(totalRamMb) : 0;

  const patch = (p: Partial<Settings>) => saveSettings({ ...settings, ...p });

  const detect = async () => {
    setDetecting(true);
    try {
      setJavaList(await api.detectJava());
    } finally {
      setDetecting(false);
    }
  };

  return (
    <div className="scroll-area h-full">
      <div className="mx-auto max-w-2xl px-8 py-6 pb-12">
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>

      {/* Appearance */}
      <Section icon={<Palette className="h-4 w-4" />} title="Appearance">
        <Field label="Theme">
          <div className="flex gap-2">
            {["dark", "light"].map((t) => (
              <button
                key={t}
                onClick={() => patch({ theme: t })}
                className={cn(
                  "flex-1 rounded-lg border px-4 py-2 text-sm font-medium capitalize transition btn-focus",
                  settings.theme === t
                    ? "border-accent bg-accent/15"
                    : "border-border bg-muted/40 hover:bg-muted",
                )}
              >
                {t}
              </button>
            ))}
          </div>
        </Field>
        <Field label="Accent color">
          <div className="flex flex-wrap gap-2">
            {Object.entries(ACCENTS).map(([name, hsl]) => (
              <button
                key={name}
                onClick={() => patch({ accent: name })}
                style={{ backgroundColor: `hsl(${hsl})` }}
                className={cn(
                  "h-8 w-8 rounded-full transition btn-focus",
                  settings.accent === name
                    ? "ring-2 ring-offset-2 ring-offset-background"
                    : "opacity-80 hover:opacity-100",
                )}
                title={name}
              />
            ))}
          </div>
        </Field>
      </Section>

      {/* Game / Java */}
      <Section icon={<Cpu className="h-4 w-4" />} title="Java & performance">
        <Field label={`Memory: ${(settings.memoryMb / 1024).toFixed(1)} GB`}>
          <input
            type="range"
            min={1024}
            max={16384}
            step={512}
            value={settings.memoryMb}
            onChange={(e) => patch({ memoryMb: Number(e.target.value) })}
            className="w-full accent-[hsl(var(--accent))]"
          />
          {recommended > 0 && (
            <div className="mt-2 flex items-center justify-between text-xs text-muted-foreground">
              <span>
                You have {(totalRamMb / 1024).toFixed(1)} GB RAM · recommended{" "}
                <span className="font-medium text-foreground">
                  {(recommended / 1024).toFixed(1)} GB
                </span>
              </span>
              {settings.memoryMb !== recommended && (
                <button
                  onClick={() => patch({ memoryMb: recommended })}
                  className="inline-flex items-center gap-1 rounded-md bg-accent/15 px-2 py-1 font-medium text-accent transition hover:bg-accent/25 btn-focus"
                >
                  <Wand2 className="h-3 w-3" /> Use recommended
                </button>
              )}
            </div>
          )}
        </Field>

        <Field label="Java executable" hint="Leave empty to auto-detect or download the right version per instance.">
          <div className="flex gap-2">
            <Input
              defaultValue={settings.javaPath ?? ""}
              onBlur={(e) => patch({ javaPath: e.target.value.trim() || null })}
              placeholder="Auto"
              className="font-mono text-xs"
            />
            <Button variant="secondary" onClick={detect} loading={detecting}>
              <Search className="h-4 w-4" /> Detect
            </Button>
          </div>
        </Field>
        {javaList && (
          <div className="space-y-1.5">
            {javaList.length === 0 && (
              <p className="text-sm text-muted-foreground">
                No Java found. One will be downloaded automatically when you launch.
              </p>
            )}
            {javaList.map((j) => (
              <button
                key={j.path}
                onClick={() => patch({ javaPath: j.path })}
                className="flex w-full items-center justify-between rounded-lg border bg-card/60 px-3 py-2 text-left text-sm transition hover:bg-muted"
              >
                <span className="truncate font-mono text-xs">{j.path}</span>
                <span className="ml-2 shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs">
                  Java {j.major}
                </span>
              </button>
            ))}
          </div>
        )}

        <JvmArgsField value={settings.jvmArgs} onCommit={(v) => patch({ jvmArgs: v })} />
      </Section>

      {/* Behavior */}
      <Section icon={<MonitorCog className="h-4 w-4" />} title="Behavior">
        <Field label={`Concurrent downloads: ${settings.maxConcurrentDownloads}`}>
          <input
            type="range"
            min={1}
            max={32}
            step={1}
            value={settings.maxConcurrentDownloads}
            onChange={(e) => patch({ maxConcurrentDownloads: Number(e.target.value) })}
            className="w-full accent-[hsl(var(--accent))]"
          />
        </Field>
        <label className="flex cursor-pointer items-center justify-between rounded-lg border bg-card/60 px-4 py-3">
          <div>
            <div className="text-sm font-medium">Hide launcher while playing</div>
            <div className="text-xs text-muted-foreground">
              Minimizes Beacon when the game starts, restores it on exit.
            </div>
          </div>
          <input
            type="checkbox"
            checked={settings.closeOnLaunch}
            onChange={(e) => patch({ closeOnLaunch: e.target.checked })}
            className="h-5 w-5 accent-[hsl(var(--accent))]"
          />
        </label>
      </Section>

      {/* Content providers */}
      <Section icon={<Package className="h-4 w-4" />} title="Content providers">
        <Field
          label="CurseForge API key"
          hint="Required to browse/install from CurseForge. Get a free key at console.curseforge.com → API Keys. Can also be set via the BEACON_CF_API_KEY env var."
        >
          <Input
            type="password"
            defaultValue={settings.curseforgeApiKey ?? ""}
            onBlur={(e) => patch({ curseforgeApiKey: e.target.value.trim() || null })}
            placeholder="$2a$10$…"
            className="font-mono text-xs"
          />
        </Field>
      </Section>

      <div className="mt-8 flex items-center gap-2 text-xs text-muted-foreground">
        <Sparkles className="h-3.5 w-3.5" />
        Beacon{version ? ` v${version}` : ""} — a modern, open-source Minecraft launcher.
      </div>
      </div>
    </div>
  );
}

/** JVM arguments editor with a one-click "Aikar's flags" preset. Controlled so
 *  applying the preset updates the textarea immediately. */
function JvmArgsField({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (v: string) => void;
}) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const isAikar = text.trim() === AIKAR_FLAGS;

  return (
    <Field
      label="Default JVM arguments"
      hint="Aikar's flags tune the garbage collector for smoother modded play. Pair them with a generous memory setting."
    >
      <div className="mb-2 flex flex-wrap items-center gap-1.5">
        <button
          type="button"
          onClick={() => {
            setText(AIKAR_FLAGS);
            onCommit(AIKAR_FLAGS);
          }}
          className={cn(
            "inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition btn-focus",
            isAikar
              ? "bg-accent/20 text-accent"
              : "bg-muted/60 text-muted-foreground hover:text-foreground",
          )}
        >
          <Wand2 className="h-3 w-3" />
          {isAikar ? "Aikar's flags applied" : "Use Aikar's flags"}
        </button>
        {text.trim() !== "" && (
          <button
            type="button"
            onClick={() => {
              setText("");
              onCommit("");
            }}
            className="rounded-md px-2 py-1 text-xs text-muted-foreground transition hover:text-foreground btn-focus"
          >
            Clear
          </button>
        )}
      </div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => onCommit(text)}
        rows={3}
        className="input-base resize-none font-mono text-xs"
      />
    </Field>
  );
}

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="mt-6">
      <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold uppercase tracking-wide text-muted-foreground">
        {icon}
        {title}
      </h2>
      <div className="space-y-4 rounded-xl border bg-card/40 p-5">{children}</div>
    </section>
  );
}

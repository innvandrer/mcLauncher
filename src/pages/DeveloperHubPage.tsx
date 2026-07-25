import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArchiveRestore,
  Box,
  Check,
  ChevronRight,
  CircleAlert,
  Code2,
  FileArchive,
  FolderCode,
  Hammer,
  Loader2,
  PackageCheck,
  Play,
  Plus,
  RefreshCw,
  Rocket,
  ScrollText,
  TestTube2,
  Trash2,
} from "lucide-react";
import { api, errMessage } from "@/lib/api";
import type {
  DeveloperInstallResult,
  DeveloperProject,
  DeveloperTaskResult,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { Button, EmptyState, Select } from "@/components/ui";
import { useStore } from "@/store/useStore";

const STORAGE_KEY = "ezmapa:developer-projects";

type RunState = {
  status: "idle" | "running" | "success" | "error";
  task?: "build" | "test";
  result?: DeveloperTaskResult;
  message?: string;
};

const copy = {
  en: {
    eyebrow: "Local development",
    title: "Developer Hub",
    subtitle:
      "Build, test and install your local mod projects without leaving EZMapa.",
    add: "Add project",
    refresh: "Refresh",
    projects: "Projects",
    clean: "Clean",
    changes: "Local changes",
    uncommitted: "Not committed yet",
    notRepository: "Not using Git",
    artifact: "Latest artifact",
    neverBuilt: "No build found",
    selectedProject: "Selected project",
    build: "Build mod",
    test: "Run tests",
    install: "Install in test profile",
    buildInstall: "Build & install",
    start: "Start Minecraft",
    chooseInstance: "Choose a test profile",
    noInstances: "Create a Minecraft instance before installing a mod.",
    pipeline: "Development pipeline",
    buildStep: "Build",
    testStep: "Test",
    installStep: "Install",
    ready: "Ready",
    waiting: "Waiting",
    logs: "Build logs",
    logsEmpty: "Run a build or test to see output here.",
    discovered: "Projects are discovered locally and stay on this computer.",
    emptyTitle: "No development projects yet",
    emptyDescription:
      "Add a Gradle, Tauri or Node project. EZMapa will also look in your usual Desktop and Documents project folders.",
    remove: "Remove project",
    installed: "Installed",
    backedUp: "Previous JAR backed up",
    buildSucceeded: "Build completed",
    testsSucceeded: "Tests completed",
    taskFailed: "Task failed",
    startHint: "Launches the selected EZMapa instance.",
    gradle: "Gradle mod",
    tauri: "Tauri app",
    node: "Node project",
  },
  no: {
    eyebrow: "Lokal utvikling",
    title: "Developer Hub",
    subtitle:
      "Bygg, test og installer lokale modprosjekter uten å forlate EZMapa.",
    add: "Legg til prosjekt",
    refresh: "Oppdater",
    projects: "Prosjekter",
    clean: "Ren",
    changes: "Lokale endringer",
    uncommitted: "Ikke committet ennå",
    notRepository: "Bruker ikke Git",
    artifact: "Nyeste bygg",
    neverBuilt: "Ingen build funnet",
    selectedProject: "Valgt prosjekt",
    build: "Bygg mod",
    test: "Kjør tester",
    install: "Installer i testprofil",
    buildInstall: "Bygg og installer",
    start: "Start Minecraft",
    chooseInstance: "Velg en testprofil",
    noInstances: "Opprett en Minecraft-instans før du installerer en mod.",
    pipeline: "Utviklingsløp",
    buildStep: "Build",
    testStep: "Test",
    installStep: "Installer",
    ready: "Klar",
    waiting: "Venter",
    logs: "Byggelogger",
    logsEmpty: "Kjør en build eller test for å se resultatet her.",
    discovered:
      "Prosjektene oppdages lokalt og informasjonen forlater ikke maskinen.",
    emptyTitle: "Ingen utviklingsprosjekter ennå",
    emptyDescription:
      "Legg til et Gradle-, Tauri- eller Node-prosjekt. EZMapa leter også i vanlige prosjektmapper på Skrivebord og Dokumenter.",
    remove: "Fjern prosjekt",
    installed: "Installert",
    backedUp: "Forrige JAR er sikkerhetskopiert",
    buildSucceeded: "Build fullført",
    testsSucceeded: "Tester fullført",
    taskFailed: "Oppgaven feilet",
    startHint: "Starter den valgte EZMapa-instansen.",
    gradle: "Gradle-mod",
    tauri: "Tauri-app",
    node: "Node-prosjekt",
  },
};

export function DeveloperHubPage() {
  const settings = useStore((s) => s.settings);
  const instances = useStore((s) => s.instances);
  const launch = useStore((s) => s.launch);
  const toast = useStore((s) => s.toast);
  const c = settings?.language === "no" ? copy.no : copy.en;

  const [projects, setProjects] = useState<DeveloperProject[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [instanceId, setInstanceId] = useState("");
  const [loading, setLoading] = useState(true);
  const [runs, setRuns] = useState<Record<string, RunState>>({});
  const [installs, setInstalls] = useState<
    Record<string, DeveloperInstallResult>
  >({});

  const selected =
    projects.find((project) => project.path === selectedPath) ??
    projects[0] ??
    null;
  const run = selected
    ? (runs[selected.path] ?? { status: "idle" as const })
    : { status: "idle" as const };
  const install = selected ? installs[selected.path] : undefined;

  useEffect(() => {
    if (!instanceId && instances.length > 0) {
      const best =
        instances.find((instance) => instance.loader === "neoforge") ??
        instances[0];
      setInstanceId(best.id);
    }
  }, [instanceId, instances]);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      setLoading(true);
      let saved: string[] = [];
      try {
        saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
      } catch {
        saved = [];
      }

      const inspected = await Promise.allSettled(
        saved.map((path) => api.inspectDeveloperProject(path)),
      );
      const known = inspected.flatMap((result) =>
        result.status === "fulfilled" ? [result.value] : [],
      );

      let discovered: DeveloperProject[] = [];
      try {
        discovered = await api.discoverDeveloperProjects();
      } catch {
        // Manual project selection still works if discovery is unavailable.
      }
      if (cancelled) return;

      const merged = mergeProjects([...known, ...discovered]);
      setProjects(merged);
      setSelectedPath((current) => current ?? merged[0]?.path ?? null);
      savePaths(merged);
      setLoading(false);
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const stats = useMemo(
    () => ({
      total: projects.length,
      buildable: projects.filter((project) => project.artifactPath).length,
      dirty: projects.filter((project) =>
        ["modified", "uncommitted"].includes(project.gitState),
      ).length,
    }),
    [projects],
  );

  const addProject = async () => {
    const selectedFolder = await openDialog({
      directory: true,
      multiple: false,
      title: c.add,
    });
    if (!selectedFolder || Array.isArray(selectedFolder)) return;
    try {
      const project = await api.inspectDeveloperProject(selectedFolder);
      setProjects((current) => {
        const next = mergeProjects([...current, project]);
        savePaths(next);
        return next;
      });
      setSelectedPath(project.path);
    } catch (error) {
      toast("error", errMessage(error));
    }
  };

  const refresh = async () => {
    setLoading(true);
    const refreshed = await Promise.allSettled(
      projects.map((project) => api.inspectDeveloperProject(project.path)),
    );
    const next = refreshed.flatMap((result) =>
      result.status === "fulfilled" ? [result.value] : [],
    );
    setProjects(next);
    savePaths(next);
    if (
      selectedPath &&
      !next.some((project) => project.path === selectedPath)
    ) {
      setSelectedPath(next[0]?.path ?? null);
    }
    setLoading(false);
  };

  const removeProject = (path: string) => {
    setProjects((current) => {
      const next = current.filter((project) => project.path !== path);
      savePaths(next);
      return next;
    });
    if (selectedPath === path) {
      setSelectedPath(
        projects.find((project) => project.path !== path)?.path ?? null,
      );
    }
  };

  const executeTask = async (
    project: DeveloperProject,
    task: "build" | "test",
  ) => {
    setRuns((current) => ({
      ...current,
      [project.path]: { status: "running", task },
    }));
    try {
      const result = await api.runDeveloperTask(project.path, task);
      setRuns((current) => ({
        ...current,
        [project.path]: {
          status: result.success ? "success" : "error",
          task,
          result,
          message: result.success
            ? task === "build"
              ? c.buildSucceeded
              : c.testsSucceeded
            : c.taskFailed,
        },
      }));
      if (result.success) {
        const refreshed = await api.inspectDeveloperProject(project.path);
        setProjects((current) =>
          current.map((item) =>
            item.path === refreshed.path ? refreshed : item,
          ),
        );
      }
      return result;
    } catch (error) {
      const message = errMessage(error);
      setRuns((current) => ({
        ...current,
        [project.path]: { status: "error", task, message },
      }));
      toast("error", message);
      return null;
    }
  };

  const installArtifact = async (project: DeveloperProject) => {
    if (!instanceId) return null;
    try {
      const result = await api.installDeveloperArtifact(
        project.path,
        instanceId,
      );
      setInstalls((current) => ({ ...current, [project.path]: result }));
      toast("success", `${c.installed}: ${result.fileName}`);
      return result;
    } catch (error) {
      toast("error", errMessage(error));
      return null;
    }
  };

  const buildAndInstall = async () => {
    if (!selected) return;
    const result = await executeTask(selected, "build");
    if (result?.success) await installArtifact(selected);
  };

  return (
    <div className="app-page">
      <div className="app-scroll app-gutter pb-8 pt-6">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="mb-1 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-accent">
              <Code2 className="h-3.5 w-3.5" />
              {c.eyebrow}
            </div>
            <h1 className="text-2xl font-bold tracking-tight">{c.title}</h1>
            <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
              {c.subtitle}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              onClick={refresh}
              loading={loading}
              title={c.refresh}
            >
              <RefreshCw className="h-4 w-4" />
              {c.refresh}
            </Button>
            <Button variant="primary" onClick={addProject}>
              <Plus className="h-4 w-4" />
              {c.add}
            </Button>
          </div>
        </header>

        <div className="mt-6 grid grid-cols-3 gap-3">
          <SummaryCard
            icon={FolderCode}
            value={stats.total}
            label={c.projects}
          />
          <SummaryCard
            icon={FileArchive}
            value={stats.buildable}
            label={c.artifact}
          />
          <SummaryCard
            icon={CircleAlert}
            value={stats.dirty}
            label={c.changes}
          />
        </div>

        {loading && projects.length === 0 ? (
          <div className="flex h-64 items-center justify-center">
            <Loader2 className="h-6 w-6 animate-spin text-accent" />
          </div>
        ) : projects.length === 0 ? (
          <EmptyState
            icon={<FolderCode className="h-7 w-7" />}
            title={c.emptyTitle}
            description={c.emptyDescription}
            action={
              <Button variant="primary" onClick={addProject}>
                <Plus className="h-4 w-4" />
                {c.add}
              </Button>
            }
          />
        ) : (
          <div className="mt-6 grid min-h-[540px] grid-cols-1 gap-5 xl:grid-cols-[330px_minmax(0,1fr)]">
            <section className="card-surface flex min-h-0 flex-col overflow-hidden">
              <div className="border-b border-border/70 px-4 py-3">
                <h2 className="text-sm font-semibold">{c.projects}</h2>
                <p className="mt-0.5 text-[11px] text-muted-foreground">
                  {c.discovered}
                </p>
              </div>
              <div className="scroll-area space-y-2 p-2">
                <AnimatePresence initial={false}>
                  {projects.map((project) => (
                    <ProjectCard
                      key={project.path}
                      project={project}
                      selected={project.path === selected?.path}
                      c={c}
                      onSelect={() => setSelectedPath(project.path)}
                      onRemove={() => removeProject(project.path)}
                    />
                  ))}
                </AnimatePresence>
              </div>
            </section>

            {selected && (
              <motion.section
                key={selected.path}
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                className="min-w-0 space-y-5"
              >
                <ProjectHeader project={selected} c={c} />

                <div className="card-surface p-5">
                  <div className="flex flex-wrap items-end gap-3">
                    <label className="min-w-[220px] flex-1 space-y-1.5">
                      <span className="text-xs font-medium text-muted-foreground">
                        {c.chooseInstance}
                      </span>
                      <Select
                        value={instanceId}
                        onChange={(event) => setInstanceId(event.target.value)}
                        disabled={instances.length === 0}
                      >
                        {instances.length === 0 && (
                          <option value="">{c.noInstances}</option>
                        )}
                        {instances.map((instance) => (
                          <option key={instance.id} value={instance.id}>
                            {instance.name} · {instance.mcVersion} ·{" "}
                            {instance.loader}
                          </option>
                        ))}
                      </Select>
                    </label>
                    <Button
                      variant="primary"
                      onClick={buildAndInstall}
                      loading={run.status === "running"}
                      disabled={
                        selected.kind !== "gradle" || instances.length === 0
                      }
                    >
                      <Rocket className="h-4 w-4" />
                      {c.buildInstall}
                    </Button>
                    <Button
                      onClick={() => instanceId && launch(instanceId)}
                      disabled={!instanceId}
                      title={c.startHint}
                    >
                      <Play className="h-4 w-4 fill-current" />
                      {c.start}
                    </Button>
                  </div>

                  <div className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-3">
                    <ActionButton
                      icon={Hammer}
                      label={c.build}
                      active={run.status === "running" && run.task === "build"}
                      onClick={() => executeTask(selected, "build")}
                    />
                    <ActionButton
                      icon={TestTube2}
                      label={c.test}
                      active={run.status === "running" && run.task === "test"}
                      onClick={() => executeTask(selected, "test")}
                    />
                    <ActionButton
                      icon={PackageCheck}
                      label={c.install}
                      disabled={
                        selected.kind !== "gradle" ||
                        !selected.artifactPath ||
                        !instanceId
                      }
                      onClick={() => installArtifact(selected)}
                    />
                  </div>
                </div>

                <Pipeline
                  c={c}
                  run={run}
                  artifactReady={Boolean(selected.artifactPath)}
                  install={install}
                />

                <LogPanel c={c} run={run} />
              </motion.section>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function SummaryCard({
  icon: Icon,
  value,
  label,
}: {
  icon: typeof FolderCode;
  value: number;
  label: string;
}) {
  return (
    <div className="card-surface flex items-center gap-3 px-4 py-3">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-accent/12 text-accent">
        <Icon className="h-4 w-4" />
      </div>
      <div className="min-w-0">
        <div className="text-xl font-bold leading-none">{value}</div>
        <div className="mt-1 truncate text-[11px] text-muted-foreground">
          {label}
        </div>
      </div>
    </div>
  );
}

function ProjectCard({
  project,
  selected,
  c,
  onSelect,
  onRemove,
}: {
  project: DeveloperProject;
  selected: boolean;
  c: (typeof copy)["en"];
  onSelect: () => void;
  onRemove: () => void;
}) {
  const tone = projectTone(project);
  return (
    <motion.div layout exit={{ opacity: 0, x: -8 }} className="group relative">
      <button
        onClick={onSelect}
        className={cn(
          "relative flex w-full items-center gap-3 overflow-hidden rounded-xl border p-3 text-left transition btn-focus",
          selected
            ? "border-accent/45 bg-accent/10"
            : "border-transparent hover:border-border hover:bg-muted/45",
        )}
      >
        {selected && (
          <span className="absolute inset-y-2 left-0 w-0.5 rounded-r bg-accent" />
        )}
        <div
          className={cn(
            "flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br text-white shadow-lg",
            tone,
          )}
        >
          {project.kind === "gradle" ? (
            <Box className="h-5 w-5" />
          ) : (
            <Code2 className="h-5 w-5" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">{project.name}</div>
          <div className="mt-0.5 flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <span>{kindLabel(project, c)}</span>
            {project.version && (
              <>
                <span>·</span>
                <span>v{project.version}</span>
              </>
            )}
          </div>
          <div
            className={cn(
              "mt-1.5 flex items-center gap-1 text-[10px]",
              gitTone(project),
            )}
          >
            <span className={cn("h-1.5 w-1.5 rounded-full", gitDot(project))} />
            {gitLabel(project, c)}
          </div>
        </div>
        <ChevronRight
          className={cn(
            "h-4 w-4 shrink-0 transition",
            selected ? "text-accent" : "text-muted-foreground",
          )}
        />
      </button>
      <button
        onClick={(event) => {
          event.stopPropagation();
          onRemove();
        }}
        className="absolute right-8 top-2 rounded-md p-1.5 text-muted-foreground opacity-0 transition hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100 btn-focus"
        title={c.remove}
      >
        <Trash2 className="h-3.5 w-3.5" />
      </button>
    </motion.div>
  );
}

function ProjectHeader({
  project,
  c,
}: {
  project: DeveloperProject;
  c: (typeof copy)["en"];
}) {
  return (
    <div className="card-surface relative overflow-hidden p-5">
      <div
        className="absolute inset-0 opacity-70"
        style={{
          background:
            "radial-gradient(70% 150% at 0% 0%, hsl(var(--accent) / 0.18), transparent 70%)",
        }}
      />
      <div className="relative flex items-start gap-4">
        <div
          className={cn(
            "flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-gradient-to-br text-white shadow-xl",
            projectTone(project),
          )}
        >
          {project.kind === "gradle" ? (
            <Box className="h-7 w-7" />
          ) : (
            <Code2 className="h-7 w-7" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[11px] font-semibold uppercase tracking-wider text-accent">
            {c.selectedProject}
          </div>
          <h2 className="truncate text-xl font-bold">{project.name}</h2>
          <p className="mt-1 truncate text-xs text-muted-foreground">
            {project.path}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1.5">
          {project.version && (
            <span className="rounded-full border border-border/80 bg-background/60 px-2.5 py-1 text-[11px] font-medium">
              v{project.version}
            </span>
          )}
          <span
            className={cn(
              "rounded-full px-2.5 py-1 text-[10px] font-medium",
              gitBadge(project),
            )}
          >
            {gitLabel(project, c)}
          </span>
        </div>
      </div>
      <div className="relative mt-5 flex items-center gap-2 rounded-xl border border-border/70 bg-background/35 px-3 py-2.5">
        <FileArchive className="h-4 w-4 text-accent" />
        <div className="min-w-0 flex-1">
          <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
            {c.artifact}
          </div>
          <div className="truncate text-xs font-medium">
            {project.artifactName ?? c.neverBuilt}
          </div>
        </div>
        {project.artifactModifiedAt && (
          <span className="text-[10px] text-muted-foreground">
            {new Date(project.artifactModifiedAt * 1000).toLocaleString()}
          </span>
        )}
      </div>
    </div>
  );
}

function ActionButton({
  icon: Icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: typeof Hammer;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled || active}
      className="flex min-h-14 items-center gap-3 rounded-xl border border-border/70 bg-muted/25 px-3 text-left transition hover:border-accent/35 hover:bg-accent/7 disabled:pointer-events-none disabled:opacity-45 btn-focus"
    >
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
        {active ? (
          <Loader2 className="h-4 w-4 animate-spin text-accent" />
        ) : (
          <Icon className="h-4 w-4" />
        )}
      </span>
      <span className="text-xs font-medium">{label}</span>
    </button>
  );
}

function Pipeline({
  c,
  run,
  artifactReady,
  install,
}: {
  c: (typeof copy)["en"];
  run: RunState;
  artifactReady: boolean;
  install?: DeveloperInstallResult;
}) {
  const buildDone =
    artifactReady || (run.task === "build" && run.status === "success");
  const testDone = run.task === "test" && run.status === "success";
  return (
    <section className="card-surface p-5">
      <h3 className="mb-4 text-sm font-semibold">{c.pipeline}</h3>
      <div className="grid grid-cols-3">
        <PipelineStep
          icon={Hammer}
          label={c.buildStep}
          state={
            buildDone ? "done" : run.task === "build" ? run.status : "idle"
          }
          detail={buildDone ? c.ready : c.waiting}
        />
        <PipelineStep
          icon={TestTube2}
          label={c.testStep}
          state={testDone ? "done" : run.task === "test" ? run.status : "idle"}
          detail={testDone ? c.ready : c.waiting}
        />
        <PipelineStep
          icon={PackageCheck}
          label={c.installStep}
          state={install ? "done" : "idle"}
          detail={install?.fileName ?? c.waiting}
          last
        />
      </div>
      {install?.backup && (
        <div className="mt-4 flex items-center gap-2 rounded-lg bg-emerald-400/8 px-3 py-2 text-[11px] text-emerald-400">
          <ArchiveRestore className="h-3.5 w-3.5" />
          {c.backedUp}
        </div>
      )}
    </section>
  );
}

function PipelineStep({
  icon: Icon,
  label,
  state,
  detail,
  last,
}: {
  icon: typeof Hammer;
  label: string;
  state: RunState["status"] | "done";
  detail: string;
  last?: boolean;
}) {
  const done = state === "done" || state === "success";
  const failed = state === "error";
  const running = state === "running";
  return (
    <div className="relative flex min-w-0 flex-col items-center px-2 text-center">
      {!last && (
        <div
          className={cn(
            "absolute left-[calc(50%+20px)] right-[calc(-50%+20px)] top-5 h-px",
            done ? "bg-emerald-400/70" : "bg-border",
          )}
        />
      )}
      <div
        className={cn(
          "relative z-10 flex h-10 w-10 items-center justify-center rounded-full border-2 bg-background",
          done
            ? "border-emerald-400 text-emerald-400"
            : failed
              ? "border-destructive text-destructive"
              : running
                ? "border-accent text-accent"
                : "border-border text-muted-foreground",
        )}
      >
        {running ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : done ? (
          <Check className="h-4 w-4" />
        ) : failed ? (
          <CircleAlert className="h-4 w-4" />
        ) : (
          <Icon className="h-4 w-4" />
        )}
      </div>
      <div className="mt-2 text-xs font-semibold">{label}</div>
      <div className="mt-0.5 max-w-full truncate text-[10px] text-muted-foreground">
        {detail}
      </div>
    </div>
  );
}

function LogPanel({ c, run }: { c: (typeof copy)["en"]; run: RunState }) {
  const output = run.result?.output ?? run.message ?? "";
  return (
    <section className="card-surface overflow-hidden">
      <div className="flex items-center justify-between border-b border-border/70 px-4 py-3">
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <ScrollText className="h-4 w-4 text-accent" />
          {c.logs}
        </h3>
        {run.result && (
          <span
            className={cn(
              "rounded-full px-2 py-0.5 text-[10px] font-semibold",
              run.result.success
                ? "bg-emerald-400/12 text-emerald-400"
                : "bg-destructive/12 text-destructive",
            )}
          >
            {(run.result.durationMs / 1000).toFixed(1)}s
          </span>
        )}
      </div>
      <div className="scroll-area h-48 bg-black/30 p-4 font-mono text-[11px] leading-relaxed">
        {run.status === "running" ? (
          <div className="flex items-center gap-2 text-accent">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            {run.task === "build" ? c.build : c.test}…
          </div>
        ) : output ? (
          <pre className="whitespace-pre-wrap break-words text-foreground/75">
            {output}
          </pre>
        ) : (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            {c.logsEmpty}
          </div>
        )}
      </div>
    </section>
  );
}

function mergeProjects(projects: DeveloperProject[]) {
  const map = new Map<string, DeveloperProject>();
  for (const project of projects) map.set(project.path, project);
  return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function savePaths(projects: DeveloperProject[]) {
  localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify(projects.map((project) => project.path)),
  );
}

function projectTone(project: DeveloperProject) {
  const name = project.name.toLowerCase();
  if (name.includes("svart")) return "from-violet-500 to-fuchsia-800";
  if (name.includes("aether")) return "from-cyan-400 to-blue-700";
  if (project.kind === "tauri") return "from-emerald-400 to-green-800";
  return "from-accent to-violet-800";
}

function kindLabel(project: DeveloperProject, c: (typeof copy)["en"]) {
  if (project.kind === "gradle") return c.gradle;
  if (project.kind === "tauri") return c.tauri;
  return c.node;
}

function gitLabel(project: DeveloperProject, c: (typeof copy)["en"]) {
  if (project.gitState === "modified") return c.changes;
  if (project.gitState === "uncommitted") return c.uncommitted;
  if (project.gitState === "notRepository") return c.notRepository;
  return c.clean;
}

function gitTone(project: DeveloperProject) {
  if (project.gitState === "modified") return "text-amber-400";
  if (project.gitState === "uncommitted") return "text-orange-400";
  if (project.gitState === "notRepository") return "text-muted-foreground";
  return "text-emerald-400";
}

function gitDot(project: DeveloperProject) {
  if (project.gitState === "modified") return "bg-amber-400";
  if (project.gitState === "uncommitted") return "bg-orange-400";
  if (project.gitState === "notRepository") return "bg-muted-foreground";
  return "bg-emerald-400";
}

function gitBadge(project: DeveloperProject) {
  if (project.gitState === "modified") return "bg-amber-400/12 text-amber-400";
  if (project.gitState === "uncommitted")
    return "bg-orange-400/12 text-orange-400";
  if (project.gitState === "notRepository")
    return "bg-muted/70 text-muted-foreground";
  return "bg-emerald-400/12 text-emerald-400";
}

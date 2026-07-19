// Mirrors the serde types in the Rust backend.

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";

export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion?: string | null;
  icon?: string | null;
  group?: string | null;
  accent?: string | null;
  favorite?: boolean;
  archived?: boolean;
  created: number;
  lastPlayed?: number | null;
  totalPlaySeconds: number;
  memoryMb?: number | null;
  javaPath?: string | null;
  jvmArgs?: string | null;
  windowWidth?: number | null;
  windowHeight?: number | null;
  envVars?: string | null;
  preLaunch?: string | null;
  postExit?: string | null;
  modCount?: number;
  packSource?: {
    provider: string;
    projectId: string;
    versionId?: string | null;
    versionName?: string | null;
  } | null;
  loadouts?: Loadout[];
  tags?: string[];
  launchProfiles?: LaunchProfile[];
}

/** A named enable/disable mod set within an instance. */
export interface Loadout {
  name: string;
  disabled: string[];
}

export interface LaunchProfile {
  name: string;
  loadout?: string | null;
  memoryMb?: number | null;
  jvmArgs?: string | null;
  quickWorld?: string | null;
  quickServer?: string | null;
}

export interface Settings {
  memoryMb: number;
  javaPath?: string | null;
  jvmArgs: string;
  theme: string;
  accent: string;
  language: "en" | "no";
  density: "comfortable" | "compact";
  sidebarCollapsed: boolean;
  reduceTransparency: boolean;
  highContrast: boolean;
  maxConcurrentDownloads: number;
  closeOnLaunch: boolean;
  autoUpdateContent: boolean;
  curseforgeApiKey?: string | null;
}

export interface PublicAccount {
  id: string;
  username: string;
  kind: string;
  active: boolean;
}

export interface VersionStub {
  id: string;
  kind: string;
  url: string;
  releaseTime: string;
  sha1: string;
}

export interface VersionList {
  latestRelease: string;
  latestSnapshot: string;
  versions: VersionStub[];
}

export interface LoaderVersion {
  version: string;
  stable: boolean;
}

export interface TaskProgress {
  id: string;
  label: string;
  stage: string;
  current: number;
  total: number;
  done: boolean;
  error?: string | null;
  bytesDone?: number | null;
  speed?: number | null;
  currentFile?: string | null;
}

export interface LogLine {
  instanceId: string;
  line: string;
  isErr: boolean;
}

export interface InstanceStateEvent {
  instanceId: string;
  running: boolean;
  exitCode?: number | null;
}

export interface AuthPrompt {
  userCode: string;
  verificationUri: string;
  message: string;
  expiresIn: number;
}

export interface ModEntry {
  fileName: string;
  enabled: boolean;
  size: number;
  projectId?: string | null;
}

export interface RemovalImpact {
  fileName: string;
  projectId?: string | null;
  requiredBy: string[];
  safe: boolean;
}

export interface JavaInstall {
  path: string;
  version: string;
  major: number;
}

// Modrinth responses are passed through verbatim (snake_case).
export interface ModHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  follows: number;
  icon_url?: string | null;
  categories: string[];
  versions: string[];
  project_type: string;
}

export interface SearchResponse {
  hits: ModHit[];
  total_hits: number;
}

export interface ResourcePackEntry {
  fileName: string;
  size: number;
  projectId?: string | null;
}

export interface ShaderEntry {
  fileName: string;
  size: number;
  projectId?: string | null;
}

export interface WorldEntry {
  name: string;
  modified?: number | null;
  size: number;
}

export interface ScreenshotEntry {
  fileName: string;
  size: number;
  takenAt: number;
}

export interface OfflineReadiness {
  ready: boolean;
  missing: string[];
}

export interface ContentVersion {
  id: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  date: string;
}

export interface ModUpdate {
  contentType: string;
  oldFileName: string;
  newFileName: string;
  versionNumber: string;
  url: string;
  sha1?: string | null;
  enabled: boolean;
  /** Which platform the update comes from. */
  source: "modrinth" | "curseforge";
  /** The project's id on `source` (Modrinth project id or CF mod id). */
  sourceProjectId?: string | null;
  /** The Modrinth version id or CurseForge file id being offered. */
  sourceVersionId?: string | null;
  /** ISO-8601 release date of the offered version. */
  date?: string | null;
  /** The file's current source-of-truth pin from the content index. */
  pinnedProvider?: string | null;
}

export interface DiskUsage {
  total: number;
  mods: number;
  saves: number;
  resourcepacks: number;
  shaders: number;
  other: number;
}

export interface Snapshot {
  fileName: string;
  world: string;
  size: number;
  created: number;
}

export interface ModConflict {
  name: string;
  files: string[];
}

export type PreflightAction =
  "increase-ram" | "lower-ram" | "open-settings" | "clean-duplicates";

export interface PreflightWarning {
  title: string;
  detail: string;
  action: PreflightAction;
  suggestedMemoryMb?: number | null;
}

export interface TurboSkip {
  label: string;
  reason: string;
}

export interface TurboResult {
  installed: string[];
  skipped: TurboSkip[];
}

export interface ModpackUpdate {
  versionId: string;
  versionName: string;
  currentVersion?: string | null;
  added: string[];
  removed: string[];
  updated: string[];
}

export interface InstallOutcome {
  file: string;
  /** Filenames of required dependencies auto-installed alongside the primary file. */
  dependencies: string[];
  /**
   * True when the file was blocked on CurseForge and fetched from Modrinth
   * instead (hash-verified identical file).
   */
  viaModrinthFallback?: boolean;
}

/** A CurseForge file the user must download manually (author blocked the API). */
export interface BlockedFileInfo {
  fileName: string;
  modName: string;
  projectId: number;
  fileId: number;
  /** CurseForge page to download the file from. */
  pageUrl: string;
}

/**
 * Emitted on `modpack://report` at the start of a CurseForge modpack install:
 * how many blocked files were re-sourced from Modrinth, and which ones need
 * manual downloading.
 */
export interface ModpackInstallReport {
  instanceId: string;
  resolvedViaModrinth: number;
  blocked: BlockedFileInfo[];
}

/** One exportable file's platform availability, for the pre-export review. */
export interface PackPreviewEntry {
  subdir: string;
  fileName: string;
  availability: "both" | "modrinth" | "curseforge" | "none";
}

export interface PackExportPreview {
  entries: PackPreviewEntry[];
}

/** A modpack export waiting on the embed/exclude review dialog. */
export interface PendingPackExport {
  instanceId: string;
  instanceName: string;
  format: "mrpack" | "cfpack" | "both";
  /** Only the entries that need a decision for this format. */
  entries: PackPreviewEntry[];
  /** Destination paths already chosen in the save dialog. */
  paths: { mrpack?: string; cfpack?: string };
}

export interface Session {
  instanceId: string;
  /** Unix seconds when the session started. */
  started: number;
  seconds: number;
}

export interface PlayerSkin {
  username: string;
  uuid: string;
  url: string;
  variant: string;
}

export interface SavedSkin {
  id: string;
  name: string;
  variant: string;
  /** "url" for a remote texture, "file" for one imported from disk. */
  kind: string;
  /** Remote texture URL (empty for file skins). */
  url: string;
  /** Local PNG path for file skins (used to re-apply). */
  path?: string | null;
  /** Data-URI preview for file skins (URL skins preview from `url`). */
  image?: string | null;
}

export interface SavedServer {
  name: string;
  ip: string;
  /** Base64 PNG (no data-URI prefix) cached by the game, if any. */
  icon?: string | null;
}

export interface ServerStatus {
  online: boolean;
  latencyMs?: number | null;
  playersOnline?: number | null;
  playersMax?: number | null;
  version?: string | null;
  motd?: string | null;
  /** Data-URI PNG favicon the server returned, if any. */
  favicon?: string | null;
  /** Forge/NeoForge mod list decoded from the handshake data, when present. */
  modInfo?: ServerModList | null;
}

export interface ServerMod {
  id: string;
  version: string;
  /** The server marked this mod as not needed on the client. */
  ignoreServerOnly: boolean;
}

export interface ServerModList {
  loader: "forge" | "neoforge" | string;
  /** The server truncated its list (very large packs). */
  truncated: boolean;
  mods: ServerMod[];
}

/** One server mod matched to a downloadable file. */
export interface PlannedServerMod {
  modId: string;
  version: string;
  provider: "modrinth" | "curseforge";
  projectId: string;
  versionId: string;
  fileName: string;
  url: string;
  sha1?: string | null;
  /** False = closest compatible version, not the server's exact one. */
  exact: boolean;
}

export interface UnresolvedServerMod {
  modId: string;
  version: string;
  searchUrl: string;
}

export interface SkippedServerMod {
  modId: string;
  reason: "platform" | "server-only" | string;
}

export interface ServerModPlan {
  address: string;
  mcVersion?: string | null;
  loader: string;
  truncated: boolean;
  resolved: PlannedServerMod[];
  unresolved: UnresolvedServerMod[];
  skipped: SkippedServerMod[];
}

export interface ServerInstanceOutcome {
  instance: Instance;
  plan: ServerModPlan;
}

/** Recommended JVM settings for an instance (Phase 5 tuner). */
export interface JvmSuggestion {
  currentArgs: string;
  suggestedArgs: string;
  currentXmxMb: number;
  suggestedXmxMb: number;
  javaMajor: number;
  systemRamMb: number;
  modCount: number;
  /** The instance has custom args — the diff is a merge, not a replace. */
  hasCustomArgs: boolean;
  reasons: string[];
}

export interface StartupGroupStat {
  fingerprint: string;
  avgMs: number;
  count: number;
}

/** Avg startup under current JVM settings vs. the previous settings. */
export interface StartupStats {
  current?: StartupGroupStat | null;
  previous?: StartupGroupStat | null;
}

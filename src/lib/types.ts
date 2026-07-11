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
  packSource?: { provider: string; projectId: string; versionId?: string | null; versionName?: string | null } | null;
  loadouts?: Loadout[];
}

/** A named enable/disable mod set within an instance. */
export interface Loadout {
  name: string;
  disabled: string[];
}

export interface Settings {
  memoryMb: number;
  javaPath?: string | null;
  jvmArgs: string;
  theme: string;
  accent: string;
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
}

export interface ScreenshotEntry {
  fileName: string;
  size: number;
  takenAt: number;
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

export type PreflightAction = "increase-ram" | "lower-ram" | "open-settings" | "clean-duplicates";

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
}

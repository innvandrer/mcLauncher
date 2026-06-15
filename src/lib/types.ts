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
  created: number;
  lastPlayed?: number | null;
  totalPlaySeconds: number;
  memoryMb?: number | null;
  javaPath?: string | null;
  jvmArgs?: string | null;
  modCount?: number;
}

export interface Settings {
  memoryMb: number;
  javaPath?: string | null;
  jvmArgs: string;
  theme: string;
  accent: string;
  maxConcurrentDownloads: number;
  closeOnLaunch: boolean;
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
  date: string;
}

export interface ModUpdate {
  oldFileName: string;
  newFileName: string;
  versionNumber: string;
  url: string;
  sha1?: string | null;
  enabled: boolean;
}

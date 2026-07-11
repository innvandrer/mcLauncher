import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuthPrompt,
  Instance,
  InstanceStateEvent,
  JavaInstall,
  Loader,
  LoaderVersion,
  ContentVersion,
  DiskUsage,
  InstallOutcome,
  LogLine,
  ModConflict,
  ModEntry,
  ModpackInstallReport,
  ModpackUpdate,
  ModUpdate,
  PlayerSkin,
  PreflightWarning,
  TurboResult,
  PublicAccount,
  ResourcePackEntry,
  SavedServer,
  SavedSkin,
  ScreenshotEntry,
  SearchResponse,
  ServerStatus,
  Session,
  Settings,
  ShaderEntry,
  Snapshot,
  TaskProgress,
  VersionList,
  WorldEntry,
} from "./types";

export const api = {
  // Versions / loaders
  listVersions: () => invoke<VersionList>("list_minecraft_versions"),
  listFabric: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_fabric_versions", { mcVersion }),
  listQuilt: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_quilt_versions", { mcVersion }),
  listForge: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_forge_versions", { mcVersion }),
  listNeoforge: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_neoforge_versions", { mcVersion }),

  // Skin
  getSkin: () => invoke<{ url: string; variant: string }>("get_skin"),
  setSkinUrl: (url: string, variant: string) =>
    invoke<void>("set_skin_url", { url, variant }),
  setSkinFile: (filePath: string, variant: string) =>
    invoke<void>("set_skin_file", { filePath, variant }),

  // Settings
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),

  // Accounts
  listAccounts: () => invoke<PublicAccount[]>("list_accounts"),
  loginMicrosoft: () => invoke<PublicAccount[]>("login_microsoft"),
  addOfflineAccount: (username: string) =>
    invoke<PublicAccount[]>("add_offline_account", { username }),
  setActiveAccount: (id: string) => invoke<PublicAccount[]>("set_active_account", { id }),
  removeAccount: (id: string) => invoke<PublicAccount[]>("remove_account", { id }),

  // Instances
  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance", { id }),
  createInstance: (args: {
    name: string;
    mcVersion: string;
    loader: Loader;
    loaderVersion?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("create_instance", args),
  updateInstance: (instance: Instance) => invoke<Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  duplicateInstance: (id: string) => invoke<Instance>("duplicate_instance", { id }),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),

  // Launch
  launchInstance: (id: string, quick?: { world?: string; server?: string }) =>
    invoke<void>("launch_instance", {
      id,
      quickWorld: quick?.world ?? null,
      quickServer: quick?.server ?? null,
    }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),
  runningInstances: () => invoke<string[]>("running_instances"),
  createShortcut: (instanceId: string, instanceName: string, world?: string | null, server?: string | null) =>
    invoke<string>("create_shortcut", {
      instanceId,
      instanceName,
      world: world ?? null,
      server: server ?? null,
    }),

  // Mods
  searchModrinth: (args: {
    query: string;
    projectType: string;
    loader?: string | null;
    gameVersion?: string | null;
    limit?: number;
    offset?: number;
  }) => invoke<SearchResponse>("search_modrinth", args),
  installMod: (args: {
    instanceId: string;
    projectId: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<InstallOutcome>("install_mod", args),
  listMods: (instanceId: string) => invoke<ModEntry[]>("list_mods", { instanceId }),
  setModEnabled: (instanceId: string, fileName: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { instanceId, fileName, enabled }),
  deleteMod: (instanceId: string, fileName: string) =>
    invoke<void>("delete_mod", { instanceId, fileName }),

  // Loadouts (named enable/disable mod sets)
  saveLoadout: (instanceId: string, name: string) =>
    invoke<Instance>("save_loadout", { instanceId, name }),
  applyLoadout: (instanceId: string, name: string) =>
    invoke<number>("apply_loadout", { instanceId, name }),
  deleteLoadout: (instanceId: string, name: string) =>
    invoke<Instance>("delete_loadout", { instanceId, name }),

  // Unified content installer (mods, resource packs, shaders)
  installContent: (args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<InstallOutcome>("install_content", args),

  // CurseForge (same SearchResponse/ModHit shape as Modrinth)
  searchCurseforge: (args: {
    query: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
    limit?: number;
    offset?: number;
  }) => invoke<SearchResponse>("search_curseforge", args),
  installCurseforgeContent: (args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<InstallOutcome>("install_curseforge_content", args),

  // Modrinth modpacks
  installMrpack: (args: {
    instanceId: string;
    projectId: string;
    versionId?: string | null;
  }) => invoke<string>("install_mrpack", args),

  // Version lists (for the version picker)
  listModrinthVersions: (projectId: string) =>
    invoke<ContentVersion[]>("list_modrinth_versions", { projectId }),
  listCurseforgeFiles: (projectId: string) =>
    invoke<ContentVersion[]>("list_curseforge_files", { projectId }),

  // Modpacks → create a new instance from a pack
  installModrinthModpack: (args: {
    projectId: string;
    versionId?: string | null;
    name?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("install_modrinth_modpack", args),
  installCurseforgeModpack: (args: {
    projectId: string;
    fileId?: string | null;
    name?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("install_curseforge_modpack", args),

  // Mod updates
  checkModUpdates: (args: {
    instanceId: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<ModUpdate[]>("check_mod_updates", args),
  applyModUpdate: (instanceId: string, update: ModUpdate) =>
    invoke<void>("apply_mod_update", { instanceId, update }),
  /** Re-pin a mod's source of truth to the other platform; returns the new project id. */
  setModSource: (instanceId: string, fileName: string, provider: "modrinth" | "curseforge") =>
    invoke<string>("set_mod_source", { instanceId, fileName, provider }),
  autoUpdateInstanceContent: (args: {
    instanceId: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<number>("auto_update_instance_content", args),

  // Export / import
  exportInstance: (id: string, dest: string) =>
    invoke<void>("export_instance", { id, dest }),
  exportMrpack: (id: string, dest: string) =>
    invoke<void>("export_mrpack", { id, dest }),
  importInstance: (src: string) => invoke<Instance>("import_instance", { src }),
  importMrpack: (src: string) => invoke<Instance>("import_mrpack", { src }),

  // Resource packs
  listResourcePacks: (instanceId: string) =>
    invoke<ResourcePackEntry[]>("list_resource_packs", { instanceId }),
  deleteResourcePack: (instanceId: string, fileName: string) =>
    invoke<void>("delete_resource_pack", { instanceId, fileName }),

  // Shaders
  listShaders: (instanceId: string) =>
    invoke<ShaderEntry[]>("list_shaders", { instanceId }),
  deleteShader: (instanceId: string, fileName: string) =>
    invoke<void>("delete_shader", { instanceId, fileName }),

  // Worlds
  listWorlds: (instanceId: string) =>
    invoke<WorldEntry[]>("list_worlds", { instanceId }),
  deleteWorld: (instanceId: string, name: string) =>
    invoke<void>("delete_world", { instanceId, name }),
  openWorldFolder: (instanceId: string, name: string) =>
    invoke<void>("open_world_folder", { instanceId, name }),

  // Screenshots
  listScreenshots: (instanceId: string) =>
    invoke<ScreenshotEntry[]>("list_screenshots", { instanceId }),
  openScreenshot: (instanceId: string, fileName: string) =>
    invoke<void>("open_screenshot", { instanceId, fileName }),

  // Disk usage
  instanceDiskUsage: (instanceId: string) =>
    invoke<DiskUsage>("instance_disk_usage", { instanceId }),

  // World snapshots / backups
  listSnapshots: (instanceId: string) =>
    invoke<Snapshot[]>("list_snapshots", { instanceId }),
  createSnapshot: (instanceId: string, world: string) =>
    invoke<Snapshot>("create_snapshot", { instanceId, world }),
  restoreSnapshot: (instanceId: string, fileName: string) =>
    invoke<void>("restore_snapshot", { instanceId, fileName }),
  deleteSnapshot: (instanceId: string, fileName: string) =>
    invoke<void>("delete_snapshot", { instanceId, fileName }),

  // Mod conflict scan
  scanModConflicts: (instanceId: string) =>
    invoke<ModConflict[]>("scan_mod_conflicts", { instanceId }),
  resolveModConflicts: (instanceId: string) =>
    invoke<string[]>("resolve_mod_conflicts", { instanceId }),
  preflightCheck: (instanceId: string) =>
    invoke<PreflightWarning[]>("preflight_check", { instanceId }),
  applyTurbo: (instanceId: string) => invoke<TurboResult>("apply_turbo", { instanceId }),

  // Java
  detectJava: () => invoke<JavaInstall[]>("detect_java"),

  // System
  systemMemoryMb: () => invoke<number>("system_memory_mb"),

  // Project body / descriptions (for detail pages)
  getModrinthProjectBody: (projectId: string) =>
    invoke<string>("get_modrinth_project_body", { projectId }),
  getCurseforgeDescription: (projectId: string) =>
    invoke<string>("get_curseforge_description", { projectId }),

  // Version-specific installs (for the version picker)
  installContentVersion: (args: {
    instanceId: string;
    projectId: string;
    versionId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<InstallOutcome>("install_content_version", args),
  installCurseforgeFile: (args: {
    instanceId: string;
    projectId: string;
    fileId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<InstallOutcome>("install_curseforge_file", args),

  // Cancel an in-flight download task (e.g. "modpack:<id>")
  cancelTask: (taskId: string) => invoke<void>("cancel_task", { taskId }),

  // Modpack updates (diff + apply)
  checkModpackUpdate: (instanceId: string) =>
    invoke<ModpackUpdate | null>("check_modpack_update", { instanceId }),
  applyModpackUpdate: (instanceId: string, versionId: string) =>
    invoke<void>("apply_modpack_update", { instanceId, versionId }),

  // Open a URL in the system browser
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  // Upload a game log to mclo.gs, returns the shareable URL
  uploadLog: (content: string) => invoke<string>("upload_log", { content }),

  // Servers — saved multiplayer list + live ping
  listServers: (instanceId: string) =>
    invoke<SavedServer[]>("list_servers", { instanceId }),
  pingServer: (address: string) => invoke<ServerStatus>("ping_server", { address }),

  // Play sessions (activity stats)
  listSessions: () => invoke<Session[]>("list_sessions"),
  savePng: (dest: string, data: number[]) => invoke<void>("save_png", { dest, data }),

  // Skin wardrobe
  listSavedSkins: () => invoke<SavedSkin[]>("list_saved_skins"),
  saveSkin: (name: string, url: string, variant: string) =>
    invoke<SavedSkin[]>("save_skin", { name, url, variant }),
  saveSkinFile: (name: string, filePath: string, variant: string) =>
    invoke<SavedSkin[]>("save_skin_file", { name, filePath, variant }),
  deleteSavedSkin: (id: string) => invoke<SavedSkin[]>("delete_saved_skin", { id }),
  fetchPlayerSkin: (query: string) =>
    invoke<PlayerSkin>("fetch_player_skin", { query }),
};

export const events = {
  onTaskProgress: (cb: (p: TaskProgress) => void): Promise<UnlistenFn> =>
    listen<TaskProgress>("task://progress", (e) => cb(e.payload)),
  onLog: (cb: (l: LogLine) => void): Promise<UnlistenFn> =>
    listen<LogLine>("instance://log", (e) => cb(e.payload)),
  onInstanceState: (cb: (s: InstanceStateEvent) => void): Promise<UnlistenFn> =>
    listen<InstanceStateEvent>("instance://state", (e) => cb(e.payload)),
  onInstanceCreated: (cb: (i: Instance) => void): Promise<UnlistenFn> =>
    listen<Instance>("instance://created", (e) => cb(e.payload)),
  onInstanceRemoved: (cb: (id: string) => void): Promise<UnlistenFn> =>
    listen<string>("instance://removed", (e) => cb(e.payload)),
  onModpackReport: (cb: (r: ModpackInstallReport) => void): Promise<UnlistenFn> =>
    listen<ModpackInstallReport>("modpack://report", (e) => cb(e.payload)),
  onAuthPrompt: (cb: (p: AuthPrompt) => void): Promise<UnlistenFn> =>
    listen<AuthPrompt>("auth://prompt", (e) => cb(e.payload)),
  onShaderInstalled: (
    cb: (p: { instanceId: string; fileName: string; version: string }) => void,
  ): Promise<UnlistenFn> =>
    listen<{ instanceId: string; fileName: string; version: string }>(
      "instance://shader-installed",
      (e) => cb(e.payload),
    ),
};

export function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as any).message);
  return String(e);
}

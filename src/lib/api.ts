import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuthPrompt,
  Instance,
  InstanceStateEvent,
  JavaInstall,
  Loader,
  LoaderVersion,
  LogLine,
  ModEntry,
  PublicAccount,
  SearchResponse,
  Settings,
  TaskProgress,
  VersionList,
} from "./types";

export const api = {
  // Versions / loaders
  listVersions: () => invoke<VersionList>("list_minecraft_versions"),
  listFabric: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_fabric_versions", { mcVersion }),
  listQuilt: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_quilt_versions", { mcVersion }),

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
  launchInstance: (id: string) => invoke<void>("launch_instance", { id }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),
  runningInstances: () => invoke<string[]>("running_instances"),

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
  }) => invoke<string>("install_mod", args),
  listMods: (instanceId: string) => invoke<ModEntry[]>("list_mods", { instanceId }),
  setModEnabled: (instanceId: string, fileName: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { instanceId, fileName, enabled }),
  deleteMod: (instanceId: string, fileName: string) =>
    invoke<void>("delete_mod", { instanceId, fileName }),

  // Java
  detectJava: () => invoke<JavaInstall[]>("detect_java"),
};

export const events = {
  onTaskProgress: (cb: (p: TaskProgress) => void): Promise<UnlistenFn> =>
    listen<TaskProgress>("task://progress", (e) => cb(e.payload)),
  onLog: (cb: (l: LogLine) => void): Promise<UnlistenFn> =>
    listen<LogLine>("instance://log", (e) => cb(e.payload)),
  onInstanceState: (cb: (s: InstanceStateEvent) => void): Promise<UnlistenFn> =>
    listen<InstanceStateEvent>("instance://state", (e) => cb(e.payload)),
  onAuthPrompt: (cb: (p: AuthPrompt) => void): Promise<UnlistenFn> =>
    listen<AuthPrompt>("auth://prompt", (e) => cb(e.payload)),
};

export function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as any).message);
  return String(e);
}

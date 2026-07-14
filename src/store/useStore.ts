import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { api, errMessage, events } from "@/lib/api";
import { applyTheme, isImageIcon } from "@/lib/utils";
import { plural, t } from "@/lib/strings";
import type {
  Instance,
  LogLine,
  ModpackInstallReport,
  PendingPackExport,
  PublicAccount,
  Settings,
  TaskProgress,
  VersionList,
} from "@/lib/types";

export type View = "home" | "instances" | "modpacks" | "accounts" | "settings";

export interface Toast {
  id: number;
  type: "info" | "success" | "error";
  message: string;
}

const LOG_CAP = 1000;

interface State {
  ready: boolean;
  view: View;
  selectedInstanceId: string | null;

  instances: Instance[];
  accounts: PublicAccount[];
  settings: Settings | null;
  versions: VersionList | null;

  running: Set<string>;
  tasks: Record<string, TaskProgress>;
  taskStarted: Record<string, number>;
  logs: Record<string, LogLine[]>;
  toasts: Toast[];
  busy: boolean;

  /** Blocked-download reports from modpack installs, keyed by instance id. */
  packReports: Record<string, ModpackInstallReport>;
  /** Instance whose blocked-mods dialog is currently shown. */
  activePackReportId: string | null;
  dismissPackReport: (instanceId: string) => void;

  /** Modpack export awaiting the embed/exclude review dialog (global modal). */
  packExport: PendingPackExport | null;
  setPackExport: (p: PendingPackExport | null) => void;
  /** True while a pack export is writing, so export buttons can disable. */
  packExporting: boolean;
  setPackExporting: (v: boolean) => void;

  update: { version: string; notes: string } | null;
  updating: boolean;
  checkForUpdates: () => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissUpdate: () => void;

  init: () => Promise<void>;
  setView: (v: View) => void;
  openInstance: (id: string) => void;
  closeInstance: () => void;

  refreshInstances: () => Promise<void>;
  createInstance: (args: {
    name: string;
    mcVersion: string;
    loader: Instance["loader"];
    loaderVersion?: string | null;
    icon?: string | null;
  }) => Promise<Instance>;
  updateInstance: (instance: Instance) => Promise<void>;
  deleteInstance: (id: string) => Promise<void>;
  duplicateInstance: (id: string) => Promise<void>;
  importInstanceFromPath: (path: string) => Promise<void>;
  launch: (id: string) => Promise<void>;
  stop: (id: string) => Promise<void>;

  refreshAccounts: () => Promise<void>;
  loginMicrosoft: () => Promise<void>;
  addOffline: (username: string) => Promise<void>;
  setActiveAccount: (id: string) => Promise<void>;
  removeAccount: (id: string) => Promise<void>;

  saveSettings: (settings: Settings) => Promise<void>;

  clearLogs: (id: string) => void;
  toast: (type: Toast["type"], message: string) => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 1;
let listenersBound = false;
// The Update handle from the updater plugin isn't serialisable for the UI, so we
// keep it module-side and expose only display fields ({version, notes}) in state.
let pendingUpdate: Update | null = null;
// Game logs stream in line-by-line and can be very chatty; buffer them and flush
// on a short timer so the store updates a handful of times per second instead of
// once per line (which previously cloned the whole log map on every line).
let pendingLogs: Record<string, LogLine[]> = {};
let logFlushScheduled = false;

export const useStore = create<State>((set, get) => ({
  ready: false,
  view: "home",
  selectedInstanceId: null,

  instances: [],
  accounts: [],
  settings: null,
  versions: null,

  running: new Set(),
  tasks: {},
  taskStarted: {},
  logs: {},
  toasts: [],
  busy: false,

  packExport: null,
  setPackExport: (p) => set({ packExport: p }),
  packExporting: false,
  setPackExporting: (v) => set({ packExporting: v }),

  packReports: {},
  activePackReportId: null,
  dismissPackReport: (instanceId) =>
    set((s) => {
      const packReports = { ...s.packReports };
      delete packReports[instanceId];
      return {
        packReports,
        activePackReportId: s.activePackReportId === instanceId ? null : s.activePackReportId,
      };
    }),

  update: null,
  updating: false,

  init: async () => {
    try {
      const [settings, accounts, instances, running] = await Promise.all([
        api.getSettings(),
        api.listAccounts(),
        api.listInstances(),
        api.runningInstances(),
      ]);
      applyTheme(settings.theme, settings.accent);
      set({
        settings,
        accounts,
        instances,
        running: new Set(running),
        ready: true,
      });

      // One-time: drop legacy auto-assigned emoji icons so existing instances
      // fall back to their mod-loader logo. The icon picker still works for
      // deliberate choices made after this runs.
      if (
        !localStorage.getItem("ezmapa:iconMigration") &&
        !localStorage.getItem("beacon:iconMigration")
      ) {
        const legacy = instances.filter((i) => i.icon && !isImageIcon(i.icon));
        if (legacy.length) {
          await Promise.all(
            legacy.map((i) => api.updateInstance({ ...i, icon: null })),
          );
          set({ instances: await api.listInstances() });
        }
        localStorage.setItem("ezmapa:iconMigration", "1");
      }
    } catch (e) {
      set({ ready: true });
      get().toast("error", errMessage(e));
    }

    // Versions can be slow / require network — load in the background.
    api
      .listVersions()
      .then((versions) => set({ versions }))
      .catch(() => {
        /* offline: handled in the create dialog */
      });

    if (!listenersBound) {
      listenersBound = true;
      events.onTaskProgress((p) => {
        set((s) => ({
          tasks: { ...s.tasks, [p.id]: p },
          taskStarted: s.taskStarted[p.id]
            ? s.taskStarted
            : { ...s.taskStarted, [p.id]: Date.now() },
        }));
        // Cancelled installs surface their own info toast + card removal; don't
        // double up with a scary error toast.
        if (p.error && !/cancel/i.test(p.error)) get().toast("error", `${p.label}: ${p.error}`);
        if (p.done) {
          // A finished modpack install turns its "installing" card into a real
          // one — pull the fully populated instance (mod count, etc.).
          if (p.id.startsWith("modpack:")) {
            get().refreshInstances();
            // Surface the blocked-downloads dialog once the install settles
            // (unless the install errored out and the card was rolled back).
            const instanceId = p.id.slice("modpack:".length);
            const report = get().packReports[instanceId];
            if (!p.error && report && report.blocked.length > 0) {
              set({ activePackReportId: instanceId });
            } else if (report) {
              get().dismissPackReport(instanceId);
            }
          }
          setTimeout(() => {
            set((s) => {
              const tasks = { ...s.tasks };
              const taskStarted = { ...s.taskStarted };
              delete tasks[p.id];
              delete taskStarted[p.id];
              return { tasks, taskStarted };
            });
          }, 1400);
        }
      });
      events.onInstanceCreated((inst) => {
        set((s) =>
          s.instances.some((i) => i.id === inst.id)
            ? s
            : { instances: [...s.instances, inst] },
        );
      });
      events.onModpackReport((report) => {
        set((s) => ({
          packReports: { ...s.packReports, [report.instanceId]: report },
        }));
        if (report.resolvedViaModrinth > 0) {
          get().toast(
            "info",
            t("install.viaModrinthToast", {
              n: report.resolvedViaModrinth,
              files: plural(report.resolvedViaModrinth, "blocked.file", "blocked.files"),
            }),
          );
        }
      });
      events.onInstanceRemoved((id) => {
        set((s) => ({
          instances: s.instances.filter((i) => i.id !== id),
          selectedInstanceId: s.selectedInstanceId === id ? null : s.selectedInstanceId,
        }));
      });
      events.onLog((line) => {
        (pendingLogs[line.instanceId] ??= []).push(line);
        if (logFlushScheduled) return;
        logFlushScheduled = true;
        setTimeout(() => {
          logFlushScheduled = false;
          const batch = pendingLogs;
          pendingLogs = {};
          set((s) => {
            const logs = { ...s.logs };
            for (const id in batch) {
              const merged = [...(logs[id] ?? []), ...batch[id]];
              if (merged.length > LOG_CAP) merged.splice(0, merged.length - LOG_CAP);
              logs[id] = merged;
            }
            return { logs };
          });
        }, 80);
      });
      events.onInstanceState((st) => {
        set((s) => {
          const running = new Set(s.running);
          if (st.running) running.add(st.instanceId);
          else running.delete(st.instanceId);
          return { running };
        });
        if (!st.running) {
          const inst = get().instances.find((i) => i.id === st.instanceId);
          const name = inst?.name ?? "Instance";
          if (st.exitCode && st.exitCode !== 0)
            get().toast("error", `${name} crashed (exit ${st.exitCode})`);
          get().refreshInstances();
        }
      });
      events.onShaderInstalled((p) => {
        get().toast(
          "success",
          `Installed ${p.fileName} for EuphoriaPatcher — relaunch to enable shaders`,
        );
        if (get().selectedInstanceId === p.instanceId) get().refreshInstances();
      });
    }

    // Check for a newer release in the background; never blocks startup.
    get().checkForUpdates();
  },

  checkForUpdates: async () => {
    try {
      const upd = await check();
      if (!upd) return;

      pendingUpdate = upd;
      get().toast("info", `Updating EZMapa to ${upd.version}…`);
      set({ updating: true });

      await upd.downloadAndInstall();
      await relaunch();
    } catch (e) {
      const msg = errMessage(e);
      const offline =
        msg.toLowerCase().includes("network") ||
        msg.toLowerCase().includes("fetch") ||
        msg.toLowerCase().includes("connect");

      if (pendingUpdate && !offline) {
        set({
          update: { version: pendingUpdate.version, notes: pendingUpdate.body ?? "" },
          updating: false,
        });
        return;
      }

      pendingUpdate = null;
      set({ updating: false });
    }
  },

  installUpdate: async () => {
    if (!pendingUpdate) return;
    set({ updating: true });
    try {
      await pendingUpdate.downloadAndInstall();
      await relaunch();
    } catch (e) {
      const msg = errMessage(e);
      const hint =
        msg.toLowerCase().includes("minisign") || msg.toLowerCase().includes("signature")
          ? " Auto-update is temporarily unavailable — download the latest installer from GitHub Releases instead."
          : "";
      get().toast("error", msg + hint);
      set({ updating: false });
    }
  },

  dismissUpdate: () => set({ update: null }),

  setView: (view) => set({ view, selectedInstanceId: null }),
  openInstance: (id) => set({ selectedInstanceId: id }),
  closeInstance: () => set({ selectedInstanceId: null }),

  refreshInstances: async () => {
    try {
      set({ instances: await api.listInstances() });
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  createInstance: async (args) => {
    const inst = await api.createInstance(args);
    await get().refreshInstances();
    get().toast("success", `Created “${inst.name}”`);
    return inst;
  },

  updateInstance: async (instance) => {
    await api.updateInstance(instance);
    await get().refreshInstances();
  },

  deleteInstance: async (id) => {
    const inst = get().instances.find((i) => i.id === id);
    try {
      await api.deleteInstance(id);
      set({ selectedInstanceId: null });
      await get().refreshInstances();
      get().toast("info", `Deleted “${inst?.name ?? id}”`);
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  duplicateInstance: async (id) => {
    await api.duplicateInstance(id);
    await get().refreshInstances();
    get().toast("success", "Instance duplicated");
  },

  importInstanceFromPath: async (path) => {
    // Instance backups come in as .zip; Modrinth modpacks as .mrpack.
    const isPack = path.toLowerCase().endsWith(".mrpack");
    try {
      get().toast("info", isPack ? "Importing modpack…" : "Importing instance…");
      const inst = isPack ? await api.importMrpack(path) : await api.importInstance(path);
      await get().refreshInstances();
      get().toast("success", `Imported “${inst.name}”`);
      set({ view: "instances", selectedInstanceId: inst.id });
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  launch: async (id) => {
    try {
      const settings = get().settings;
      const inst = get().instances.find((i) => i.id === id);
      if (settings?.autoUpdateContent && inst && !inst.packSource) {
        try {
          const count = await api.autoUpdateInstanceContent({
            instanceId: inst.id,
            loader: inst.loader,
            gameVersion: inst.mcVersion,
          });
          if (count > 0) {
            get().toast(
              "success",
              `Updated ${count} mod${count > 1 ? "s" : ""}/pack${count > 1 ? "s" : ""} before launch`,
            );
            await get().refreshInstances();
          }
        } catch {
          /* offline or nothing to update — launch anyway */
        }
      }
      set((s) => ({ logs: { ...s.logs, [id]: [] } }));
      await api.launchInstance(id);
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  stop: async (id) => {
    try {
      await api.stopInstance(id);
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  refreshAccounts: async () => {
    try {
      set({ accounts: await api.listAccounts() });
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  loginMicrosoft: async () => {
    set({ busy: true });
    try {
      const accounts = await api.loginMicrosoft();
      set({ accounts });
      get().toast("success", "Signed in to Microsoft account");
    } catch (e) {
      get().toast("error", errMessage(e));
    } finally {
      set({ busy: false });
    }
  },

  addOffline: async (username) => {
    try {
      set({ accounts: await api.addOfflineAccount(username) });
      get().toast("success", `Added offline account “${username}”`);
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  setActiveAccount: async (id) => {
    try {
      set({ accounts: await api.setActiveAccount(id) });
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  removeAccount: async (id) => {
    try {
      set({ accounts: await api.removeAccount(id) });
    } catch (e) {
      get().toast("error", errMessage(e));
    }
  },

  saveSettings: async (settings) => {
    await api.saveSettings(settings);
    applyTheme(settings.theme, settings.accent);
    set({ settings });
  },

  clearLogs: (id) => set((s) => ({ logs: { ...s.logs, [id]: [] } })),

  toast: (type, message) => {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { id, type, message }] }));
    setTimeout(() => get().dismissToast(id), type === "error" ? 6000 : 3500);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

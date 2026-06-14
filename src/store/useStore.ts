import { create } from "zustand";
import { api, errMessage, events } from "@/lib/api";
import { applyTheme } from "@/lib/utils";
import type {
  AuthPrompt,
  Instance,
  LogLine,
  PublicAccount,
  Settings,
  TaskProgress,
  VersionList,
} from "@/lib/types";

export type View = "instances" | "accounts" | "settings";

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
  logs: Record<string, LogLine[]>;
  toasts: Toast[];
  authPrompt: AuthPrompt | null;
  busy: boolean;

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
  launch: (id: string) => Promise<void>;
  stop: (id: string) => Promise<void>;

  refreshAccounts: () => Promise<void>;
  loginMicrosoft: () => Promise<void>;
  addOffline: (username: string) => Promise<void>;
  setActiveAccount: (id: string) => Promise<void>;
  removeAccount: (id: string) => Promise<void>;
  dismissAuthPrompt: () => void;

  saveSettings: (settings: Settings) => Promise<void>;

  clearLogs: (id: string) => void;
  toast: (type: Toast["type"], message: string) => void;
  dismissToast: (id: number) => void;
}

let toastSeq = 1;
let listenersBound = false;

export const useStore = create<State>((set, get) => ({
  ready: false,
  view: "instances",
  selectedInstanceId: null,

  instances: [],
  accounts: [],
  settings: null,
  versions: null,

  running: new Set(),
  tasks: {},
  logs: {},
  toasts: [],
  authPrompt: null,
  busy: false,

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
        set((s) => ({ tasks: { ...s.tasks, [p.id]: p } }));
        if (p.error) get().toast("error", `${p.label}: ${p.error}`);
        if (p.done) {
          setTimeout(() => {
            set((s) => {
              const tasks = { ...s.tasks };
              delete tasks[p.id];
              return { tasks };
            });
          }, 1400);
        }
      });
      events.onLog((line) => {
        set((s) => {
          const prev = s.logs[line.instanceId] ?? [];
          const next = [...prev, line];
          if (next.length > LOG_CAP) next.splice(0, next.length - LOG_CAP);
          return { logs: { ...s.logs, [line.instanceId]: next } };
        });
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
      events.onAuthPrompt((p) => set({ authPrompt: p }));
    }
  },

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
    await api.deleteInstance(id);
    set({ selectedInstanceId: null });
    await get().refreshInstances();
    get().toast("info", `Deleted “${inst?.name ?? id}”`);
  },

  duplicateInstance: async (id) => {
    await api.duplicateInstance(id);
    await get().refreshInstances();
    get().toast("success", "Instance duplicated");
  },

  launch: async (id) => {
    try {
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
    set({ accounts: await api.listAccounts() });
  },

  loginMicrosoft: async () => {
    set({ busy: true });
    try {
      const accounts = await api.loginMicrosoft();
      set({ accounts, authPrompt: null });
      get().toast("success", "Signed in to Microsoft account");
    } catch (e) {
      get().toast("error", errMessage(e));
    } finally {
      set({ busy: false, authPrompt: null });
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
    set({ accounts: await api.setActiveAccount(id) });
  },

  removeAccount: async (id) => {
    set({ accounts: await api.removeAccount(id) });
  },

  dismissAuthPrompt: () => set({ authPrompt: null }),

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

import { useEffect } from "react";
import { motion } from "framer-motion";
import { TitleBar } from "./components/TitleBar";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Toaster } from "./components/Toaster";
import { CommandPalette } from "./components/CommandPalette";
import { DropZone } from "./components/DropZone";
import { AuthPromptModal } from "./components/AuthPromptModal";
import { BlockedModsModal } from "./components/BlockedModsModal";
import { UpdateModal } from "./components/UpdateModal";
import { OnboardingModal } from "./components/OnboardingModal";
import { ShortcutsOverlay } from "./components/ShortcutsOverlay";
import { Spinner } from "./components/ui";
import { HomePage } from "./pages/HomePage";
import { InstancesPage } from "./pages/InstancesPage";
import { InstanceDetailPage } from "./pages/InstanceDetailPage";
import { ModpacksPage } from "./pages/ModpacksPage";
import { AccountsPage } from "./pages/AccountsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useStore } from "./store/useStore";

export default function App() {
  const ready = useStore((s) => s.ready);
  const init = useStore((s) => s.init);
  const view = useStore((s) => s.view);
  const selectedInstanceId = useStore((s) => s.selectedInstanceId);

  useEffect(() => {
    init();
  }, [init]);

  // A key that changes whenever the visible screen changes (for transitions).
  const screenKey = selectedInstanceId ? `inst:${selectedInstanceId}` : view;

  return (
    <div className="flex h-full w-full min-h-0 flex-col overflow-hidden">
      <TitleBar />
      <div className="flex min-h-0 min-w-0 flex-1 overflow-hidden">
        <Sidebar />
        <main className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
          {!ready ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
              <Spinner className="h-6 w-6" />
              <span className="text-sm">Loading EZMapa…</span>
            </div>
          ) : (
            <motion.div
              key={screenKey}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.18, ease: "easeOut" }}
              className="h-full min-h-0 min-w-0"
            >
              <ErrorBoundary>
                {selectedInstanceId ? (
                  <InstanceDetailPage id={selectedInstanceId} />
                ) : view === "home" ? (
                  <HomePage />
                ) : view === "instances" ? (
                  <InstancesPage />
                ) : view === "modpacks" ? (
                  <ModpacksPage />
                ) : view === "accounts" ? (
                  <AccountsPage />
                ) : (
                  <SettingsPage />
                )}
              </ErrorBoundary>
            </motion.div>
          )}
        </main>
      </div>
      <Toaster />
      <CommandPalette />
      <DropZone />
      <AuthPromptModal />
      <BlockedModsModal />
      <UpdateModal />
      <OnboardingModal />
      <ShortcutsOverlay />
    </div>
  );
}

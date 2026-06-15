import { useEffect } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { TitleBar } from "./components/TitleBar";
import { Sidebar } from "./components/Sidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Toaster } from "./components/Toaster";
import { AuthPromptModal } from "./components/AuthPromptModal";
import { Spinner } from "./components/ui";
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
    <div className="flex h-screen w-screen flex-col overflow-hidden">
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="relative flex-1 overflow-hidden">
          {!ready ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
              <Spinner className="h-6 w-6" />
              <span className="text-sm">Loading Beacon…</span>
            </div>
          ) : (
            <AnimatePresence mode="wait">
              <motion.div
                key={screenKey}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.18, ease: "easeOut" }}
                className="h-full"
              >
                <ErrorBoundary>
                  {selectedInstanceId ? (
                    <InstanceDetailPage id={selectedInstanceId} />
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
            </AnimatePresence>
          )}
        </main>
      </div>
      <Toaster />
      <AuthPromptModal />
    </div>
  );
}

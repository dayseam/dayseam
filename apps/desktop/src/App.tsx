import { useCallback, useEffect, useState } from "react";
import { ActionBar } from "./components/ActionBar";
import { Footer } from "./components/Footer";
import { LogDrawer } from "./components/LogDrawer";
import { ReportPreview } from "./components/ReportPreview";
import { TitleBar } from "./components/TitleBar";
import { ToastHost } from "./components/ToastHost";
import { IdentityManagerDialog } from "./features/identities/IdentityManagerDialog";
import { SinksDialog } from "./features/sinks/SinksDialog";
import { SourcesSidebar } from "./features/sources/SourcesSidebar";
import { dismissSplash } from "./splash";
import { ThemeProvider } from "./theme";

export default function App() {
  const [logsOpen, setLogsOpen] = useState(false);
  const [identitiesOpen, setIdentitiesOpen] = useState(false);
  const [sinksOpen, setSinksOpen] = useState(false);

  const toggleLogs = useCallback(() => setLogsOpen((prev) => !prev), []);
  const closeLogs = useCallback(() => setLogsOpen(false), []);

  // Remove the inline splash defined in `index.html` the moment the
  // app has rendered. Running this in an effect (rather than at
  // module scope) guarantees the user sees the rendered UI at least
  // one frame before the splash starts fading, so there's no
  // "splash gone, nothing in its place" flicker.
  useEffect(() => {
    dismissSplash();
  }, []);

  // ⌘L (macOS) / Ctrl+L (Linux/Windows) toggles the log drawer.
  // Tauri already blocks the browser's "focus address bar" default
  // for Ctrl+L inside a webview, so we only need to guard against
  // our own listener firing when a text field is focused.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const isMod = event.metaKey || event.ctrlKey;
      if (!isMod || event.key.toLowerCase() !== "l") return;
      const target = event.target as HTMLElement | null;
      if (target && /^(input|textarea|select)$/i.test(target.tagName)) return;
      event.preventDefault();
      toggleLogs();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggleLogs]);

  return (
    <ThemeProvider>
      <div className="flex min-h-screen flex-col bg-white text-neutral-900 dark:bg-neutral-950 dark:text-neutral-100">
        <TitleBar />
        <ActionBar />
        <SourcesSidebar />
        <ReportPreview />
        <Footer
          onOpenLogs={toggleLogs}
          onOpenIdentities={() => setIdentitiesOpen(true)}
          onOpenSinks={() => setSinksOpen(true)}
        />
      </div>
      <LogDrawer open={logsOpen} onClose={closeLogs} />
      <IdentityManagerDialog
        open={identitiesOpen}
        onClose={() => setIdentitiesOpen(false)}
      />
      <SinksDialog open={sinksOpen} onClose={() => setSinksOpen(false)} />
      <ToastHost />
    </ThemeProvider>
  );
}

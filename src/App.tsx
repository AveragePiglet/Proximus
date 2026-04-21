import { useState, useCallback, useRef, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { TitleBar } from "./components/TitleBar";
import { Toolbar } from "./components/Toolbar";
import { Sidebar } from "./components/Sidebar";
import { Terminal, TerminalHandle } from "./components/Terminal";
import { TabBar } from "./components/TabBar";
import { ProjectsView } from "./components/ProjectsView";
import { StatusBar } from "./components/StatusBar";
import { QuickActions } from "./components/QuickActions";
import MigrationDialog from "./components/MigrationDialog";
import SettingsDialog from "./components/SettingsDialog";
import { useTabStore } from "./hooks/useTabStore";
import "./styles/global.css";

function App() {
  const {
    activeTabs,
    activeProjectTabs,
    activeChatTabs,
    closedTabs,
    activeTabId,
    migrationPending,
    dismissMigration,
    toast,
    createTab,
    createScratchTab,
    closeTab,
    switchTab,
    reopenTab,
    removeTab,
  } = useTabStore();

  const [showProjects, setShowProjects] = useState(!activeTabId);
  const [recovering, setRecovering] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [pendingAuthCode, setPendingAuthCode] = useState<string | null>(null);
  const [pendingAuthUrl, setPendingAuthUrl] = useState<string | null>(null);
  const terminalRef = useRef<TerminalHandle | null>(null);
  const proxyAuthUrlOpenedRef = useRef(false);

  // Global listener for proxy-initiated device flow (active even when Settings is closed)
  useEffect(() => {
    let unlistenOutput: (() => void) | null = null;

    listen<string>("copilot-auth-output", (e) => {
      const line = e.payload;
      const codeMatch = line.match(/"([A-Z0-9]{4}-[A-Z0-9]{4})"/);
      const urlMatch = line.match(/(https:\/\/github\.com\/login\/device[^\s]*)/);
      if (codeMatch) setPendingAuthCode(codeMatch[1]);
      if (urlMatch && !proxyAuthUrlOpenedRef.current) {
        proxyAuthUrlOpenedRef.current = true;
        setPendingAuthUrl(urlMatch[1]);
        setShowSettings(true);
        openUrl(urlMatch[1]).catch(() => {});
      }
    }).then(u => { unlistenOutput = u; });

    return () => { unlistenOutput?.(); };
  }, []);

  const handleSwitch = useCallback(async (tabId: string) => {
    await switchTab(tabId);
    setShowProjects(false);
  }, [switchTab]);

  const handleCreateProject = useCallback(async () => {
    const tabId = await createTab();
    if (tabId) setShowProjects(false);
    return tabId;
  }, [createTab]);

  const handleCreateChat = useCallback(async () => {
    const tabId = await createScratchTab();
    if (tabId) setShowProjects(false);
    return tabId;
  }, [createScratchTab]);

  const handleReopen = useCallback(async (tabId: string) => {
    await reopenTab(tabId);
    setShowProjects(false);
  }, [reopenTab]);

  return (
    <div className="app">
      <TitleBar />
      <Toolbar />
      <TabBar
        tabs={activeTabs}
        activeTabId={showProjects ? null : activeTabId}
        onSwitch={handleSwitch}
        onClose={closeTab}
        onNew={() => setShowProjects(true)}
      />
      <div className="main-content">
        {showProjects || !activeTabId ? (
          <ProjectsView
            activeProjectTabs={activeProjectTabs}
            activeChatTabs={activeChatTabs}
            closedTabs={closedTabs}
            onCreateProject={handleCreateProject}
            onCreateChat={handleCreateChat}
            onSwitchTab={handleSwitch}
            onReopenTab={handleReopen}
            onRemoveTab={removeTab}
            onSettings={() => setShowSettings(true)}
          />
        ) : (
          <div style={{ display: "flex", flexDirection: "column", flex: 1, overflow: "hidden" }}>
            <Terminal tabId={activeTabId} ref={terminalRef} locked={recovering} />
            <QuickActions tabId={activeTabId} terminalRef={terminalRef} onRecoveringChange={setRecovering} tabType={activeTabs.find(t => t.id === activeTabId)?.tab_type ?? "chat"} />
            <StatusBar tabId={activeTabId} />
          </div>
        )}
        {!showProjects && activeTabId && <Sidebar tabId={activeTabId} />}
      </div>
      {migrationPending && (
        <MigrationDialog
          projectPath={migrationPending.projectPath}
          detectedFiles={migrationPending.files}
          tabId={migrationPending.tabId}
          onComplete={dismissMigration}
        />
      )}
      {showSettings && (
        <SettingsDialog
          initialAuthCode={pendingAuthCode}
          initialAuthUrl={pendingAuthUrl}
          onClose={() => {
            setShowSettings(false);
            setPendingAuthCode(null);
            setPendingAuthUrl(null);
            proxyAuthUrlOpenedRef.current = false;
          }}
        />
      )}
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

export default App;

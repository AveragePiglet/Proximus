import { useState, useCallback, useRef } from "react";
import { Toolbar } from "./components/Toolbar";
import { Sidebar } from "./components/Sidebar";
import { Terminal, TerminalHandle } from "./components/Terminal";
import { TabBar } from "./components/TabBar";
import { ProjectsView } from "./components/ProjectsView";
import { StatusBar } from "./components/StatusBar";
import { QuickActions } from "./components/QuickActions";
import { useTabStore } from "./hooks/useTabStore";
import "./styles/global.css";

function App() {
  const {
    activeTabs,
    activeProjectTabs,
    activeChatTabs,
    closedTabs,
    activeTabId,
    createTab,
    createScratchTab,
    closeTab,
    switchTab,
    reopenTab,
    removeTab,
  } = useTabStore();

  const [showProjects, setShowProjects] = useState(!activeTabId);
  const [recovering, setRecovering] = useState(false);
  const terminalRef = useRef<TerminalHandle | null>(null);

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
    </div>
  );
}

export default App;

import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface TabState {
  id: string;
  project_path: string;
  project_name: string;
  status: string; // "active" | "closed"
  tab_type: string; // "project" | "chat"
  last_opened: string;
  created_at: string;
}

export function useTabStore() {
  const [tabs, setTabs] = useState<TabState[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const allTabs = await invoke<TabState[]>("get_tabs");
      const activeId = await invoke<string | null>("get_active_tab_id");
      setTabs(allTabs);
      setActiveTabId(activeId);
    } catch (e) {
      console.error("Failed to refresh tabs:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const createTab = useCallback(async (projectPath?: string) => {
    let path = projectPath;
    if (!path) {
      const selected = await open({ directory: true, title: "Select Project Folder" });
      if (!selected) return null;
      path = selected as string;
    }
    setLoading(true);
    try {
      const tabId = await invoke<string>("create_tab", { projectPath: path });
      await refresh();
      setLoading(false);
      return tabId;
    } catch (e) {
      setLoading(false);
      throw e;
    }
  }, [refresh]);

  const closeTab = useCallback(async (tabId: string) => {
    // Switch to this tab first, then close (backend closes active tab)
    await invoke("switch_tab", { tabId });
    await invoke("close_tab");
    await refresh();
  }, [refresh]);

  const switchTab = useCallback(async (tabId: string) => {
    await invoke("switch_tab", { tabId });
    setActiveTabId(tabId);
  }, []);

  const createScratchTab = useCallback(async () => {
    setLoading(true);
    try {
      const tabId = await invoke<string>("create_scratch_tab");
      await refresh();
      setLoading(false);
      return tabId;
    } catch (e) {
      setLoading(false);
      throw e;
    }
  }, [refresh]);

  const reopenTab = useCallback(async (tabId: string) => {
    await invoke("reopen_tab", { tabId });
    await refresh();
  }, [refresh]);

  const removeTab = useCallback(async (tabId: string) => {
    await invoke("remove_tab", { tabId });
    await refresh();
  }, [refresh]);

  const activeTabs = tabs.filter((t) => t.status === "active");
  const closedTabs = tabs.filter((t) => t.status === "closed");
  const activeProjectTabs = activeTabs.filter((t) => t.tab_type === "project");
  const activeChatTabs = activeTabs.filter((t) => t.tab_type === "chat");

  return {
    tabs,
    activeTabs,
    activeProjectTabs,
    activeChatTabs,
    closedTabs,
    activeTabId,
    loading,
    createTab,
    createScratchTab,
    closeTab,
    switchTab,
    reopenTab,
    removeTab,
    refresh,
  };
}

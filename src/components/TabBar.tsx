import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { TabState } from "../hooks/useTabStore";

interface TabBarProps {
  tabs: TabState[];
  activeTabId: string | null;
  onSwitch: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onNew: () => void;
}

export const TabBar: React.FC<TabBarProps> = ({ tabs, activeTabId, onSwitch, onClose, onNew }) => {
  if (tabs.length === 0 && !activeTabId) return null;

  return (
    <div className="tab-bar">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tab-item${tab.id === activeTabId ? " active" : ""}${tab.tab_type === "chat" ? " chat" : ""}`}
          onClick={() => onSwitch(tab.id)}
        >
          <span className="tab-name">
            {tab.tab_type === "chat" ? `💬 ${tab.project_name}` : tab.project_name}
          </span>
          {tab.tab_type !== "chat" && tab.project_path && (
            <button
              className="tab-folder"
              onClick={(e) => {
                e.stopPropagation();
                invoke("open_project_folder", { path: tab.project_path });
              }}
              title="Open project folder"
            >
              📂
            </button>
          )}
          <button
            className="tab-close"
            onClick={(e) => {
              e.stopPropagation();
              onClose(tab.id);
            }}
            title="Close tab"
          >
            ×
          </button>
        </div>
      ))}
      <button className="tab-new" onClick={onNew} title="Projects">
        +
      </button>
    </div>
  );
};

import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TabState } from "../hooks/useTabStore";

const openFolder = (path: string, e: React.MouseEvent) => {
  e.stopPropagation();
  invoke("open_project_folder", { path }).catch(() => {});
};

interface ProjectsViewProps {
  activeProjectTabs: TabState[];
  activeChatTabs: TabState[];
  closedTabs: TabState[];
  onCreateProject: () => Promise<string | null>;
  onCreateChat: () => Promise<string | null>;
  onSwitchTab: (tabId: string) => void;
  onReopenTab: (tabId: string) => void;
  onRemoveTab: (tabId: string) => void;
  onSettings: () => void;
}

const SectionHeader: React.FC<{ label: string }> = ({ label }) => (
  <div className="projects-section-title">
    <h3>{label}</h3>
    <div className="projects-section-line" />
  </div>
);

export const ProjectsView: React.FC<ProjectsViewProps> = ({
  activeProjectTabs,
  activeChatTabs,
  closedTabs,
  onCreateProject,
  onCreateChat,
  onSwitchTab,
  onReopenTab,
  onRemoveTab,
  onSettings,
}) => {
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async (fn: () => Promise<string | null>) => {
    setCreating(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    }
    setCreating(false);
  };

  const closedProjects = closedTabs.filter((t) => t.tab_type === "project");
  const closedChats = closedTabs.filter((t) => t.tab_type === "chat");
  const isEmpty =
    activeProjectTabs.length === 0 &&
    activeChatTabs.length === 0 &&
    closedTabs.length === 0;

  return (
    <div className="projects-view">
      {/* ── Header ── */}
      <div className="projects-header">
        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <h2>Workspace</h2>
          <span style={{ fontSize: 11.5, color: "var(--text-tertiary)", fontWeight: 400 }}>
            Open a project folder to start a Claude session
          </span>
        </div>
        <div className="projects-header-actions">
          <button
            className="btn btn-start"
            onClick={() => handleCreate(onCreateProject)}
            disabled={creating}
          >
            {creating ? "Opening…" : "+ New Project"}
          </button>
          <button
            className="btn btn-chat"
            onClick={() => handleCreate(onCreateChat)}
            disabled={creating}
          >
            ✦ New Chat
          </button>
          <button className="btn btn-settings" onClick={onSettings} title="Settings">
            ⚙ Settings
          </button>
        </div>
      </div>

      {/* ── Body ── */}
      <div className="projects-body">
        {error && <div className="projects-error">{error}</div>}

        {isEmpty && (
          <div className="projects-empty">
            <div className="projects-empty-icon">⚡</div>
            <p>No projects yet.</p>
            <p>Click "+ New Project" to open a folder, or "✦ New Chat" for a quick conversation.</p>
          </div>
        )}

        {activeProjectTabs.length > 0 && (
          <div className="projects-section">
            <SectionHeader label="Active Projects" />
            <div className="projects-grid">
              {activeProjectTabs.map((tab) => (
                <div
                  key={tab.id}
                  className="project-card"
                  onClick={() => onSwitchTab(tab.id)}
                >
                  <div className="project-card-name">{tab.project_name}</div>
                  <div className="project-card-path">{tab.project_path}</div>
                  <div className="project-card-actions">
                    <button
                      className="btn btn-small btn-folder"
                      onClick={(e) => openFolder(tab.project_path, e)}
                      title="Open in file explorer"
                    >
                      Open Folder
                    </button>
                  </div>
                </div>
              ))}
              {/* Ghost "new project" tile */}
              <div
                className="project-card-new"
                onClick={() => handleCreate(onCreateProject)}
              >
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                  <line x1="7" y1="2" x2="7" y2="12" />
                  <line x1="2" y1="7" x2="12" y2="7" />
                </svg>
                New project
              </div>
            </div>
          </div>
        )}

        {activeChatTabs.length > 0 && (
          <div className="projects-section">
            <SectionHeader label="Active Chats" />
            <div className="projects-grid">
              {activeChatTabs.map((tab) => (
                <div
                  key={tab.id}
                  className="project-card chat"
                  onClick={() => onSwitchTab(tab.id)}
                >
                  <div className="project-card-name">✦ {tab.project_name}</div>
                </div>
              ))}
            </div>
          </div>
        )}

        {closedProjects.length > 0 && (
          <div className="projects-section">
            <SectionHeader label="Recent Projects" />
            <div className="projects-grid">
              {closedProjects.map((tab) => (
                <div key={tab.id} className="project-card closed">
                  <div className="project-card-name">{tab.project_name}</div>
                  <div className="project-card-path">{tab.project_path}</div>
                  <div className="project-card-actions">
                    <button
                      className="btn btn-small btn-reopen"
                      onClick={() => onReopenTab(tab.id)}
                    >
                      Reopen
                    </button>
                    <button
                      className="btn btn-small btn-folder"
                      onClick={(e) => openFolder(tab.project_path, e)}
                      title="Open in file explorer"
                    >
                      Open Folder
                    </button>
                    <button
                      className="btn btn-small btn-stop"
                      onClick={() => onRemoveTab(tab.id)}
                    >
                      Remove
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {closedChats.length > 0 && (
          <div className="projects-section">
            <SectionHeader label="Recent Chats" />
            <div className="projects-grid">
              {closedChats.map((tab) => (
                <div key={tab.id} className="project-card chat closed">
                  <div className="project-card-name">✦ {tab.project_name}</div>
                  <div className="project-card-actions">
                    <button
                      className="btn btn-small btn-reopen"
                      onClick={() => onReopenTab(tab.id)}
                    >
                      Reopen
                    </button>
                    <button
                      className="btn btn-small btn-stop"
                      onClick={() => onRemoveTab(tab.id)}
                    >
                      Remove
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

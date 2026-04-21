import React, { useState } from "react";
import { TabState } from "../hooks/useTabStore";

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

  return (
    <div className="projects-view">
      <div className="projects-header">
        <h2>Projects</h2>
        <div className="projects-header-actions">
          <button className="btn btn-start" onClick={() => handleCreate(onCreateProject)} disabled={creating}>
            {creating ? "Opening..." : "+ New Project"}
          </button>
          <button className="btn btn-chat" onClick={() => handleCreate(onCreateChat)} disabled={creating}>
            💬 New Chat
          </button>
          <button className="btn btn-settings" onClick={onSettings} title="Settings">
            ⚙ Settings
          </button>
        </div>
      </div>

      {error && <div className="projects-error">{error}</div>}

      {activeProjectTabs.length > 0 && (
        <div className="projects-section">
          <h3>Active Projects</h3>
          <div className="projects-grid">
            {activeProjectTabs.map((tab) => (
              <div key={tab.id} className="project-card" onClick={() => onSwitchTab(tab.id)}>
                <div className="project-card-name">{tab.project_name}</div>
                <div className="project-card-path">{tab.project_path}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {activeChatTabs.length > 0 && (
        <div className="projects-section">
          <h3>Active Chats</h3>
          <div className="projects-grid">
            {activeChatTabs.map((tab) => (
              <div key={tab.id} className="project-card chat" onClick={() => onSwitchTab(tab.id)}>
                <div className="project-card-name">💬 {tab.project_name}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {closedProjects.length > 0 && (
        <div className="projects-section">
          <h3>Recent Projects</h3>
          <div className="projects-grid">
            {closedProjects.map((tab) => (
              <div key={tab.id} className="project-card closed">
                <div className="project-card-name">{tab.project_name}</div>
                <div className="project-card-path">{tab.project_path}</div>
                <div className="project-card-actions">
                  <button className="btn btn-small btn-reopen" onClick={() => onReopenTab(tab.id)}>
                    Reopen
                  </button>
                  <button className="btn btn-small btn-stop" onClick={() => onRemoveTab(tab.id)}>
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
          <h3>Recent Chats</h3>
          <div className="projects-grid">
            {closedChats.map((tab) => (
              <div key={tab.id} className="project-card chat closed">
                <div className="project-card-name">💬 {tab.project_name}</div>
                <div className="project-card-actions">
                  <button className="btn btn-small btn-reopen" onClick={() => onReopenTab(tab.id)}>
                    Reopen
                  </button>
                  <button className="btn btn-small btn-stop" onClick={() => onRemoveTab(tab.id)}>
                    Remove
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {activeProjectTabs.length === 0 && activeChatTabs.length === 0 && closedTabs.length === 0 && (
        <div className="projects-empty">
          <p>No projects yet.</p>
          <p>Click "+ New Project" to open a folder, or "💬 New Chat" for a quick conversation.</p>
        </div>
      )}

    </div>
  );
};

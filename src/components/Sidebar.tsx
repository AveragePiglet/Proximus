import React, { useState, useCallback, useRef, useEffect } from "react";
import { MemoryGraphView } from "./MemoryGraphView";
import { NodeDetail } from "./NodeDetail";
import { LogsPanel } from "./LogsPanel";
import { SettingsPanel } from "./SettingsPanel";
import { useMemoryGraph, MemoryNode } from "../hooks/useMemoryGraph";

type TabId = "graph" | "detail" | "state" | "logs" | "settings";

// SVG icons for the activity bar
const TabIcons: Record<TabId, React.ReactNode> = {
  graph: (
    <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="3.5" cy="8.5" r="2" />
      <circle cx="13.5" cy="3.5" r="2" />
      <circle cx="13.5" cy="13.5" r="2" />
      <line x1="5.4" y1="7.4" x2="11.6" y2="4.6" />
      <line x1="5.4" y1="9.6" x2="11.6" y2="12.4" />
    </svg>
  ),
  detail: (
    <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2.5" y="2.5" width="12" height="12" rx="2" />
      <line x1="5.5" y1="6" x2="11.5" y2="6" />
      <line x1="5.5" y1="8.5" x2="11.5" y2="8.5" />
      <line x1="5.5" y1="11" x2="9" y2="11" />
    </svg>
  ),
  state: (
    <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8.5" cy="8.5" r="5.5" />
      <polyline points="8.5,5.5 8.5,8.5 11,10" />
    </svg>
  ),
  logs: (
    <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2.5" y="3" width="12" height="11" rx="2" />
      <line x1="5" y1="6.5" x2="12" y2="6.5" />
      <line x1="5" y1="9" x2="12" y2="9" />
      <line x1="5" y1="11.5" x2="9" y2="11.5" />
    </svg>
  ),
  settings: (
    <svg width="17" height="17" viewBox="0 0 17 17" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8.5" cy="8.5" r="5.5" />
      <circle cx="8.5" cy="8.5" r="2" fill="currentColor" stroke="none" />
    </svg>
  ),
};

const TABS: { id: TabId; label: string }[] = [
  { id: "graph", label: "Memory Graph" },
  { id: "logs", label: "Logs" },
  { id: "settings", label: "Theme" },
];

const MIN_WIDTH = 200;
const MAX_WIDTH = 800;
const DEFAULT_WIDTH = 340;

interface SidebarProps {
  tabId: string | null;
}

export const Sidebar: React.FC<SidebarProps> = ({ tabId }) => {
  const { graph, state, refresh } = useMemoryGraph(tabId);
  const [selectedNode, setSelectedNode] = useState<MemoryNode | null>(null);
  const [activeTab, setActiveTab] = useState<TabId | null>(null);
  const [panelWidth, setPanelWidth] = useState(DEFAULT_WIDTH);
  const isDragging = useRef(false);
  const startX = useRef(0);
  const startWidth = useRef(0);

  const handleNodeSelect = useCallback(
    (nodeId: string) => {
      const node = graph.nodes.find((n) => n.id === nodeId) || null;
      setSelectedNode(node);
      setActiveTab("detail");
    },
    [graph]
  );

  const toggleTab = (tabId: TabId) => {
    setActiveTab((prev) => (prev === tabId ? null : tabId));
  };

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      isDragging.current = true;
      startX.current = e.clientX;
      startWidth.current = panelWidth;
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    },
    [panelWidth]
  );

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      // Panel is on the right, drag handle on the left edge of the panel
      // Moving mouse left = wider panel, moving right = narrower
      const delta = startX.current - e.clientX;
      const newWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startWidth.current + delta));
      setPanelWidth(newWidth);
    };

    const onMouseUp = () => {
      if (!isDragging.current) return;
      isDragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, []);

  return (
    <div className="sidebar-wrapper">
      {/* Flyout Panel */}
      {activeTab && (
        <>
          {/* Resize handle */}
          <div className="side-panel-resize-handle" onMouseDown={onMouseDown} />
          <div className="side-panel" style={{ width: panelWidth }}>
            {activeTab === "graph" && (
              <div className="side-panel-content">
                <div className="sidebar-section-header">
                  <h3 className="side-panel-title">Memory Graph</h3>
                  <button className="btn-icon" onClick={refresh} title="Refresh">↻</button>
                </div>
                {graph.nodes.length > 0 ? (
                  <MemoryGraphView graph={graph} onNodeSelect={handleNodeSelect} />
                ) : (
                  <div className="sidebar-placeholder">
                    <p>No nodes found</p>
                    <p>Memory graph is empty</p>
                  </div>
                )}
              </div>
            )}

            {activeTab === "detail" && (
              <div className="side-panel-content">
                <h3 className="side-panel-title">Node Detail</h3>
                <NodeDetail node={selectedNode} tabId={tabId} />
              </div>
            )}

            {activeTab === "state" && state && (
              <div className="side-panel-content">
                <h3 className="side-panel-title">State</h3>
                <div className="state-info">
                  {state.active_task && (
                    <div className="state-field">
                      <label>Task</label>
                      <span>{state.active_task}</span>
                    </div>
                  )}
                  <div className="state-field">
                    <label>Branch</label>
                    <span>{state.branch}</span>
                  </div>
                  {state.next_action && (
                    <div className="state-field">
                      <label>Next</label>
                      <span>{state.next_action}</span>
                    </div>
                  )}
                </div>
              </div>
            )}

            {activeTab === "logs" && <LogsPanel />}

            {activeTab === "settings" && <SettingsPanel />}
          </div>
        </>
      )}

      {/* Activity Bar */}
      <div className="activity-bar">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            className={`activity-tab${activeTab === tab.id ? " active" : ""}`}
            onClick={() => toggleTab(tab.id)}
            title={tab.label}
          >
            <span className="activity-tab-icon">{TabIcons[tab.id]}</span>
          </button>
        ))}
      </div>
    </div>
  );
};

import React, { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const TitleBar: React.FC = () => {
  const [maximized, setMaximized] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    appWindow.isMaximized().then(setMaximized);
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const handleMinimize = useCallback(() => appWindow.minimize(), []);
  const handleMaximize = useCallback(() => appWindow.toggleMaximize(), []);
  const handleClose = useCallback(() => appWindow.close(), []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-left" data-tauri-drag-region>
        <span className="titlebar-icon" data-tauri-drag-region>⚡</span>
        <span className="titlebar-label" data-tauri-drag-region>Proximus</span>
      </div>
      <div className="titlebar-controls">
        <button className="titlebar-btn titlebar-minimize" onClick={handleMinimize} title="Minimize">
          <svg width="10" height="1" viewBox="0 0 10 1"><rect width="10" height="1" fill="currentColor" /></svg>
        </button>
        <button className="titlebar-btn titlebar-maximize" onClick={handleMaximize} title={maximized ? "Restore" : "Maximize"}>
          {maximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
              <rect x="2" y="0" width="8" height="8" rx="1" />
              <rect x="0" y="2" width="8" height="8" rx="1" fill="var(--bg-secondary)" />
              <rect x="0" y="2" width="8" height="8" rx="1" />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
              <rect x="0.5" y="0.5" width="9" height="9" rx="1" />
            </svg>
          )}
        </button>
        <button className="titlebar-btn titlebar-close" onClick={handleClose} title="Close">
          <svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" strokeWidth="1.2">
            <line x1="0" y1="0" x2="10" y2="10" />
            <line x1="10" y1="0" x2="0" y2="10" />
          </svg>
        </button>
      </div>
    </div>
  );
};

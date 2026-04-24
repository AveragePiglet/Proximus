import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DependencyDialogProps {
  claudeMissing: boolean;
  copilotApiMissing: boolean;
  copilotCliMissing: boolean;
  onResolved: () => void;
  onSkip: () => void;
}

export default function DependencyDialog({
  claudeMissing,
  copilotApiMissing,
  copilotCliMissing,
  onResolved,
  onSkip,
}: DependencyDialogProps) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleInstall = useCallback(async () => {
    setInstalling(true);
    setError(null);
    try {
      await invoke("install_dependencies", {
        installClaude: claudeMissing,
        installCopilotApi: copilotApiMissing,
        installCopilotCli: copilotCliMissing,
      });
      onResolved();
    } catch (e) {
      setError(String(e));
      setInstalling(false);
    }
  }, [claudeMissing, copilotApiMissing, copilotCliMissing, onResolved]);

  return (
    <div className="migration-overlay">
      <div className="migration-dialog">
        <h3>Missing Dependencies</h3>
        <p className="migration-subtitle">
          The following packages are required but not installed:
        </p>

        <div className="dep-list">
          {claudeMissing && (
            <div className="dep-item">
              <span className="dep-name">Claude Code CLI</span>
              <code className="dep-cmd">npm install -g @anthropic-ai/claude-code</code>
            </div>
          )}
          {copilotApiMissing && (
            <div className="dep-item">
              <span className="dep-name">Copilot API</span>
              <code className="dep-cmd">npm install -g copilot-api</code>
            </div>
          )}
          {copilotCliMissing && (
            <div className="dep-item">
              <span className="dep-name">Copilot CLI</span>
              <code className="dep-cmd">npm install -g @github/copilot</code>
            </div>
          )}
        </div>

        {error && <p className="dep-error">{error}</p>}

        <div className="dep-actions">
          <button
            className="btn btn-start"
            onClick={handleInstall}
            disabled={installing}
          >
            {installing ? "Installing..." : "Install"}
          </button>
          <button
            className="btn dep-btn-skip"
            onClick={onSkip}
            disabled={installing}
          >
            Skip
          </button>
        </div>
      </div>
    </div>
  );
}

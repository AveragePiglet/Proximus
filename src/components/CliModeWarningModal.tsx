import { useState } from "react";

interface CliModeWarningModalProps {
  targetMode: "claude" | "copilot";
  projectPath: string | null;
  onConfirm: (syncFiles: boolean) => void;
  onCancel: () => void;
}

export default function CliModeWarningModal({
  targetMode,
  projectPath,
  onConfirm,
  onCancel,
}: CliModeWarningModalProps) {
  const [syncFiles, setSyncFiles] = useState(true);

  const modeName =
    targetMode === "copilot"
      ? "Copilot CLI"
      : "Claude CLI + Copilot Proxy";

  const hasProject = !!projectPath;

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div
        className="modal-dialog cli-mode-warning-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="modal-title">⚠ Switch CLI Mode?</h3>

        <p className="modal-body">
          Switching to <strong>{modeName}</strong> will:
        </p>
        <ul className="modal-list">
          <li>Close all open tabs</li>
          {targetMode === "copilot" && (
            <li>Stop the Copilot Proxy and model-rewrite proxy</li>
          )}
          {targetMode === "claude" && (
            <li>Start the Copilot Proxy and model-rewrite proxy</li>
          )}
        </ul>

        {hasProject && (
          <label className="modal-checkbox-row">
            <input
              type="checkbox"
              checked={syncFiles}
              onChange={(e) => setSyncFiles(e.target.checked)}
            />
            <span>
              Sync project files to{" "}
              <strong>
                {targetMode === "copilot" ? "Copilot" : "Claude"}
              </strong>{" "}
              conventions
            </span>
          </label>
        )}

        <div className="modal-actions">
          <button className="btn btn-start" onClick={() => onConfirm(syncFiles)}>
            Switch &amp; Close Tabs
          </button>
          <button className="btn btn-secondary" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

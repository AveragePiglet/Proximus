import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MigrationDialogProps {
  projectPath: string;
  detectedFiles: string[];
  tabId: string;
  onComplete: () => void;
}

export default function MigrationDialog({
  projectPath,
  detectedFiles,
  tabId,
  onComplete,
}: MigrationDialogProps) {
  const [migrating, setMigrating] = useState(false);
  const [status, setStatus] = useState("");

  const handleMigrate = async () => {
    setMigrating(true);
    setStatus("Reading existing memory files...");
    try {
      // 1. Scaffold the fresh .node-memory structure
      await invoke("scaffold_project_cmd", { projectPath });

      // 2. Ensure all expected dirs/files are present (catches template additions)
      await invoke("sync_memory_structure", { projectPath });

      // 3. Update CLAUDE.md to reference new memory system
      await invoke("update_claude_md_references", { projectPath });

      // 4. Read contents of detected files
      const contents = await invoke<[string, string][]>(
        "get_migration_file_contents",
        { projectPath, files: detectedFiles }
      );

      // 5. Build migration prompt and write it to the PTY
      const filesSummary = contents
        .map(([path, content]) => `--- ${path} ---\n${content}`)
        .join("\n\n");

      const migrationPrompt = `I've just scaffolded a fresh .node-memory/ system for this project and updated the CLAUDE.md to reference the new memory system. The project had existing memory/context files that need to be migrated into the new TOML-based graph memory system.

Please do the following:
1. Read the existing memory files below
2. Migrate their content into .node-memory/ (graph.toml nodes/edges, invariants.toml rules, nodes/*.toml for detailed info)
3. Review the CLAUDE.md file — ensure it references .node-memory/ correctly and remove any remaining references to the old memory system (memory/, .cursorrules, etc.)
4. Follow the memory protocol defined in CLAUDE.md

Existing memory files:
${filesSummary}

Migrate this into the .node-memory/ system now.`;

      await invoke("write_pty", { tabId, data: migrationPrompt });
      // Wait for Claude Code to fully process the bracketed paste before sending Enter
      await new Promise((r) => setTimeout(r, 2000));
      await invoke("write_pty", { tabId, data: "\r" });

      setStatus("Migration started — Claude is converting your memory...");
      setTimeout(onComplete, 1500);
    } catch (e) {
      setStatus(`Error: ${e}`);
      setMigrating(false);
    }
  };

  const handleFresh = async () => {
    try {
      await invoke("scaffold_project_cmd", { projectPath });
      // Ensure all expected dirs/files exist (catches template additions)
      await invoke("sync_memory_structure", { projectPath });
      onComplete();
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  const handleSkip = () => {
    onComplete();
  };

  return (
    <div className="migration-overlay" onClick={handleSkip}>
      <div className="migration-dialog" onClick={(e) => e.stopPropagation()}>
        <h3>Existing project memory detected</h3>
        <p className="migration-subtitle">
          This project has AI memory/context files that can be migrated to the
          structured .node-memory system.
        </p>
        <div className="migration-files">
          {detectedFiles.map((f) => (
            <div key={f} className="migration-file-item">
              📄 {f}
            </div>
          ))}
        </div>

        {status && <div className="migration-status">{status}</div>}

        <div className="migration-actions">
          <button
            className="btn btn-start"
            onClick={handleMigrate}
            disabled={migrating}
          >
            {migrating ? "Migrating..." : "Migrate Memory"}
          </button>
          <button
            className="btn"
            onClick={handleFresh}
            disabled={migrating}
          >
            Start Fresh
          </button>
          <button
            className="btn btn-small"
            onClick={handleSkip}
            disabled={migrating}
          >
            Skip
          </button>
        </div>
      </div>
    </div>
  );
}

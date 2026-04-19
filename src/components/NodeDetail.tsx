import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MemoryNode } from "../hooks/useMemoryGraph";

interface NodeDetailProps {
  node: MemoryNode | null;
  tabId: string | null;
}

export const NodeDetail: React.FC<NodeDetailProps> = ({ node, tabId }) => {
  const [content, setContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!node) {
      setContent(null);
      return;
    }

    // Try to load the node file
    setLoading(true);
    invoke<string>("get_node_content", { tabId, nodeId: node.id })
      .then((c) => {
        setContent(c);
        setLoading(false);
      })
      .catch(() => {
        setContent(null);
        setLoading(false);
      });
  }, [node]);

  if (!node) {
    return (
      <div className="node-detail">
        <div className="node-detail-empty">Click a node to view details</div>
      </div>
    );
  }

  return (
    <div className="node-detail">
      <div className="node-detail-header">
        <span className={`node-type-badge ${node.node_type}`}>
          {node.node_type}
        </span>
        <span className="node-id">{node.id}</span>
      </div>

      <div className="node-detail-field">
        <label>L0</label>
        <span>{node.l0}</span>
      </div>

      <div className="node-detail-field">
        <label>L1</label>
        <span>{node.l1}</span>
      </div>

      <div className="node-detail-field">
        <label>Score</label>
        <div className="score-bar">
          <div
            className="score-fill"
            style={{ width: `${node.score * 100}%` }}
          />
          <span>{node.score.toFixed(1)}</span>
        </div>
      </div>

      <div className="node-detail-field">
        <label>Last touched</label>
        <span>{node.last_touched}</span>
      </div>

      {loading && <div className="node-detail-loading">Loading...</div>}

      {content && (
        <div className="node-detail-content">
          <label>Node file</label>
          <pre>{content}</pre>
        </div>
      )}

      {!content && !loading && (
        <div className="node-detail-field">
          <label>L2</label>
          <span className="node-l2">{node.l2}</span>
        </div>
      )}
    </div>
  );
};

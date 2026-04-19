import React from "react";

interface StatusBadgeProps {
  name: string;
  running: boolean;
  port: number | null;
}

export const StatusBadge: React.FC<StatusBadgeProps> = ({ name, running, port }) => {
  return (
    <div className="status-badge">
      <span className={`status-dot ${running ? "running" : "stopped"}`} />
      <span className="status-name">{name}</span>
      {port && <span className="status-port">:{port}</span>}
    </div>
  );
};

import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export interface ProcessStatus {
  name: string;
  running: boolean;
  port: number | null;
}

export function useProcessStatus() {
  const [statuses, setStatuses] = useState<ProcessStatus[]>([
    { name: "copilot-proxy", running: false, port: 4141 },
    { name: "model-rewriter", running: false, port: 4142 },
    { name: "claude", running: false, port: null },
  ]);

  useEffect(() => {
    // Listen for process status events from Rust
    const unlisten = listen<ProcessStatus>("process-status", (event) => {
      setStatuses((prev) =>
        prev.map((s) =>
          s.name === event.payload.name ? { ...s, ...event.payload } : s
        )
      );
    });

    // Poll initial statuses
    invoke<ProcessStatus[]>("get_process_statuses").then((result) => {
      setStatuses((prev) =>
        prev.map((s) => {
          const update = result.find((r) => r.name === s.name);
          return update ? { ...s, ...update } : s;
        })
      );
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return statuses;
}

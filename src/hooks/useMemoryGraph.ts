import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface MemoryNode {
  id: string;
  node_type: string;
  l0: string;
  l1: string;
  l2: string;
  score: number;
  last_touched: string;
}

export interface MemoryEdge {
  from: string;
  to: string;
  rel: string;
}

export interface MemoryGraph {
  nodes: MemoryNode[];
  edges: MemoryEdge[];
}

export interface MemoryState {
  active_task: string;
  branch: string;
  next_action: string;
}

interface MemoryChangedEvent {
  tab_id: string;
  kind: string;
}

export function useMemoryGraph(tabId: string | null) {
  const [graph, setGraph] = useState<MemoryGraph>({ nodes: [], edges: [] });
  const [state, setState] = useState<MemoryState | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!tabId) return;
    try {
      const g = await invoke<MemoryGraph>("get_memory_graph", { tabId });
      setGraph(g);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    try {
      const s = await invoke<MemoryState>("get_memory_state", { tabId });
      setState(s);
    } catch (_) {}
  }, [tabId]);

  useEffect(() => {
    if (!tabId) {
      setGraph({ nodes: [], edges: [] });
      setState(null);
      return;
    }

    refresh();

    // Re-fetch when memory files change for this tab
    const unlisten = listen<MemoryChangedEvent>("memory-changed", (event) => {
      if (event.payload.tab_id === tabId) {
        refresh();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [tabId, refresh]);

  return { graph, state, error, refresh };
}

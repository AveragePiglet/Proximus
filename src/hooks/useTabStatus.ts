import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface TabStatus {
  pty_running: boolean;
  pty_started_at: string | null;
  last_memory_save: string | null;
  context_percent: number;
  tokens_used: number;
  tokens_total: number;
  cost_usd: number;
  model: string;
}

interface ProximusStats {
  used_percentage: number;
  context_window_size: number;
  total_input_tokens: number;
  total_output_tokens: number;
  cost_usd: number;
  model: string;
}

const EMPTY_STATUS: TabStatus = {
  pty_running: false,
  pty_started_at: null,
  last_memory_save: null,
  context_percent: 0,
  tokens_used: 0,
  tokens_total: 200000,
  cost_usd: 0,
  model: "",
};

export function useTabStatus(tabId: string | null) {
  const [status, setStatus] = useState<TabStatus>(EMPTY_STATUS);

  const fetchAll = useCallback(async () => {
    if (!tabId) return;
    try {
      const s = await invoke<TabStatus>("get_tab_status", { tabId });

      // Read context from ~/.claude/proximus-stats/ (written by statusline.sh)
      try {
        const ctx = await invoke<ProximusStats>("get_context_usage", { tabId });
        s.context_percent = ctx.used_percentage ?? 0;
        s.tokens_total = ctx.context_window_size ?? 200000;
        // Derive tokens_used from the percentage so the displayed number
        // matches what /context reports (total_input_tokens is cumulative, not current)
        s.tokens_used = Math.round((s.context_percent / 100) * s.tokens_total);
        s.cost_usd = ctx.cost_usd ?? 0;
        s.model = ctx.model ?? "";
      } catch {
        // stats file may not exist yet
      }

      setStatus(s);
    } catch {
      // tab may not exist yet
    }
  }, [tabId]);

  // Reset and re-fetch when tabId changes; poll every 5s
  useEffect(() => {
    setStatus(EMPTY_STATUS);
    fetchAll();
    const interval = setInterval(fetchAll, 5_000);
    return () => clearInterval(interval);
  }, [fetchAll]);

  // Listen for memory-changed events to update last_memory_save
  useEffect(() => {
    if (!tabId) return;
    const unlisten = listen<{ tab_id: string; timestamp: string }>("memory-changed", (event) => {
      if (event.payload.tab_id === tabId) {
        setStatus((prev) => ({ ...prev, last_memory_save: event.payload.timestamp }));
        invoke("update_memory_save_time", { tabId, timestamp: event.payload.timestamp }).catch(() => {});
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [tabId]);

  return status;
}

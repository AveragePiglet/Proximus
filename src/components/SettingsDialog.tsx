import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import CliModeWarningModal from "./CliModeWarningModal";

interface SettingsDialogProps {
  onClose: () => void;
  initialAuthCode?: string | null;
  initialAuthUrl?: string | null;
}

interface ModelEntry {
  id: string;
  display_name: string;
  tier: "opus" | "sonnet" | "haiku";
}

interface AppSettings {
  project_primary_model: string;
  project_secondary_model: string;
  chat_model: string;
  dangerously_skip_permissions: boolean;
  cli_mode: "claude" | "copilot";
  copilot_model: string;
  openrouter_api_key: string;
  openrouter_model: string;
}

const TIER_LABELS: Record<string, string> = {
  opus:   "Opus — deep reasoning & planning",
  sonnet: "Sonnet — coding & general work",
  haiku:  "Haiku — fast & lightweight",
};

// ── Account section ──────────────────────────────────────────────

function AccountSection({ initialAuthCode, initialAuthUrl, openrouterActive }: {
  initialAuthCode?: string | null;
  initialAuthUrl?: string | null;
  openrouterActive: boolean;
}) {
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [authState, setAuthState] = useState<"idle" | "waiting" | "done" | "error">(
    initialAuthCode || initialAuthUrl ? "waiting" : "idle"
  );
  const [deviceCode, setDeviceCode] = useState<string | null>(initialAuthCode ?? null);
  const [deviceUrl, setDeviceUrl] = useState<string | null>(initialAuthUrl ?? null);

  // Cancel any pending auth process when this component unmounts (dialog closed)
  useEffect(() => {
    return () => {
      invoke("cancel_copilot_auth").catch(() => {});
    };
  }, []);

  useEffect(() => {
    invoke<boolean>("get_copilot_auth_status").then(setAuthed);
  }, []);

  useEffect(() => {
    let unlistenOutput: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;

    listen<string>("copilot-auth-output", (e) => {
      const line = e.payload;
      const codeMatch = line.match(/([A-Z0-9]{4}-[A-Z0-9]{4})/i);
      const urlMatch  = line.match(/(https:\/\/github\.com\/login\/device[^\s]*)/);
      if (codeMatch) setDeviceCode(codeMatch[1]);
      if (urlMatch) {
        setDeviceUrl(urlMatch[1]);
        // Browser is opened by App.tsx's global listener — not here
      }
    }).then(u => { unlistenOutput = u; });

    listen<boolean>("copilot-auth-done", (e) => {
      if (e.payload) {
        setAuthState("done");
        setAuthed(true);
      } else {
        setAuthState("error");
      }
    }).then(u => { unlistenDone = u; });

    return () => {
      unlistenOutput?.();
      unlistenDone?.();
    };
  }, []);

  const handleSignIn = useCallback(async () => {
    setAuthState("waiting");
    setDeviceCode(null);
    setDeviceUrl(null);
    try {
      await invoke("start_copilot_auth");
    } catch (e) {
      setAuthState("error");
    }
  }, []);

  const handleSignOut = useCallback(async () => {
    try {
      await invoke("sign_out_copilot");
      setAuthed(false);
      setAuthState("idle");
    } catch (e) {
      console.error("Sign out failed:", e);
    }
  }, []);

  return (
    <section className="settings-section-block">
      <h4 className="settings-section-title">GitHub Copilot Auth</h4>
      <div className="settings-section-divider" />

      {openrouterActive && (
        <div className="settings-or-bypass-notice">
          <span className="settings-or-bypass-icon">⚡</span>
          <span>
            <strong>OpenRouter is active</strong> — GitHub authentication is not used in Claude mode while an OpenRouter API key is set.
            Switch to <strong>Copilot CLI</strong> mode to use GitHub auth instead.
          </span>
        </div>
      )}

      <div className="settings-account-row">
        <div className="settings-account-info">
          <span className={`settings-account-badge ${authed ? "authed" : "unauthed"}`}>
            {authed === null ? "…" : authed ? "● Connected" : "○ Not connected"}
          </span>
          <span className="settings-field-desc">
            {authed
              ? "GitHub Copilot is authenticated and ready."
              : "Sign in with GitHub to enable the Copilot proxy."}
          </span>
        </div>
        {!authed && authState === "idle" && (
          <button className="btn-account-action" onClick={handleSignIn}>
            Sign in with GitHub
          </button>
        )}
        {authed && (
          <div className="settings-account-actions">
            <button className="btn-account-action" onClick={handleSignIn}>
              Re-authenticate
            </button>
            <button className="btn-account-action btn-account-danger" onClick={handleSignOut}>
              Sign out
            </button>
          </div>
        )}
      </div>

      {authState === "waiting" && (
        <div className="settings-auth-flow">
          {deviceCode ? (
            <>
              <p className="settings-auth-instruction">
                A browser window has opened. Enter this code on GitHub:
              </p>
              <div className="settings-device-code">{deviceCode}</div>
              {deviceUrl && (
                <button
                  className="btn btn-small"
                  onClick={() => deviceUrl && openUrl(deviceUrl).catch(() => {})}
                >
                  Open GitHub ↗
                </button>
              )}
              <p className="settings-auth-waiting">Waiting for authorisation…</p>
            </>
          ) : (
            <p className="settings-auth-waiting">Starting auth flow…</p>
          )}
        </div>
      )}

      {authState === "done" && (
        <p className="settings-auth-success">✓ Successfully authenticated with GitHub Copilot.</p>
      )}

      {authState === "error" && (
        <p className="settings-auth-error">Authentication failed. Please try again.</p>
      )}
    </section>
  );
}

// ── OpenRouter section ───────────────────────────────────────────

// Cost relative to Claude Sonnet 3.5 ($3/M input) as a rough "×" multiplier shown in UI.
// Format: "context — $X/M in · $Y/M out"
const OR_MODELS = [
  { group: "Anthropic", models: [
    { id: "anthropic/claude-opus-4-5",              label: "Claude Opus 4.5",         ctx: "200K", cost: "$15/$75" },
    { id: "anthropic/claude-sonnet-4-5",            label: "Claude Sonnet 4.5",       ctx: "200K", cost: "$3/$15" },
    { id: "anthropic/claude-haiku-4-5",             label: "Claude Haiku 4.5",        ctx: "200K", cost: "$0.80/$4" },
    { id: "anthropic/claude-opus-4-0",              label: "Claude Opus 4",           ctx: "200K", cost: "$15/$75" },
    { id: "anthropic/claude-sonnet-4-0",            label: "Claude Sonnet 4",         ctx: "200K", cost: "$3/$15" },
    { id: "anthropic/claude-3-7-sonnet",            label: "Claude 3.7 Sonnet",       ctx: "200K", cost: "$3/$15" },
    { id: "anthropic/claude-3-5-sonnet",            label: "Claude 3.5 Sonnet",       ctx: "200K", cost: "$3/$15" },
    { id: "anthropic/claude-3-5-haiku",             label: "Claude 3.5 Haiku",        ctx: "200K", cost: "$0.80/$4" },
    { id: "anthropic/claude-3-opus",                label: "Claude 3 Opus",           ctx: "200K", cost: "$15/$75" },
  ]},
  { group: "OpenAI", models: [
    { id: "openai/gpt-5",                           label: "GPT-5",                   ctx: "1M",   cost: "$10/$40" },
    { id: "openai/gpt-4.1",                         label: "GPT-4.1",                 ctx: "1M",   cost: "$2/$8" },
    { id: "openai/gpt-4.1-mini",                    label: "GPT-4.1 Mini",            ctx: "1M",   cost: "$0.40/$1.60" },
    { id: "openai/gpt-4.1-nano",                    label: "GPT-4.1 Nano",            ctx: "1M",   cost: "$0.10/$0.40" },
    { id: "openai/gpt-4o",                          label: "GPT-4o",                  ctx: "128K", cost: "$2.50/$10" },
    { id: "openai/gpt-4o-mini",                     label: "GPT-4o Mini",             ctx: "128K", cost: "$0.15/$0.60" },
    { id: "openai/o3",                              label: "o3",                      ctx: "200K", cost: "$10/$40" },
    { id: "openai/o3-mini",                         label: "o3-mini",                 ctx: "200K", cost: "$1.10/$4.40" },
    { id: "openai/o4-mini",                         label: "o4-mini",                 ctx: "200K", cost: "$1.10/$4.40" },
    { id: "openai/o1",                              label: "o1",                      ctx: "200K", cost: "$15/$60" },
  ]},
  { group: "Google", models: [
    { id: "google/gemini-2.5-pro",                  label: "Gemini 2.5 Pro",          ctx: "1M",   cost: "$1.25/$10" },
    { id: "google/gemini-2.5-flash",                label: "Gemini 2.5 Flash",        ctx: "1M",   cost: "$0.15/$0.60" },
    { id: "google/gemini-2.5-flash-thinking",       label: "Gemini 2.5 Flash Think",  ctx: "1M",   cost: "$0.15/$3.50" },
    { id: "google/gemini-2.0-flash-001",            label: "Gemini 2.0 Flash",        ctx: "1M",   cost: "$0.10/$0.40" },
    { id: "google/gemini-2.0-flash-lite-001",       label: "Gemini 2.0 Flash Lite",   ctx: "1M",   cost: "$0.075/$0.30" },
    { id: "google/gemini-1.5-pro",                  label: "Gemini 1.5 Pro",          ctx: "2M",   cost: "$1.25/$5" },
    { id: "google/gemini-1.5-flash",                label: "Gemini 1.5 Flash",        ctx: "1M",   cost: "$0.075/$0.30" },
  ]},
  { group: "Meta", models: [
    { id: "meta-llama/llama-4-maverick",            label: "Llama 4 Maverick",        ctx: "1M",   cost: "$0.18/$0.60" },
    { id: "meta-llama/llama-4-scout",               label: "Llama 4 Scout",           ctx: "512K", cost: "$0.08/$0.30" },
    { id: "meta-llama/llama-3.3-70b-instruct",      label: "Llama 3.3 70B",           ctx: "128K", cost: "$0.12/$0.30" },
    { id: "meta-llama/llama-3.1-405b-instruct",     label: "Llama 3.1 405B",          ctx: "128K", cost: "$2.70/$2.70" },
  ]},
  { group: "DeepSeek", models: [
    { id: "deepseek/deepseek-r2",                   label: "DeepSeek R2",             ctx: "164K", cost: "$0.55/$2.19" },
    { id: "deepseek/deepseek-chat-v3-0324",         label: "DeepSeek V3",             ctx: "164K", cost: "$0.27/$1.10" },
    { id: "deepseek/deepseek-r1",                   label: "DeepSeek R1",             ctx: "164K", cost: "$0.55/$2.19" },
  ]},
  { group: "Mistral", models: [
    { id: "mistralai/mistral-large",                label: "Mistral Large",           ctx: "128K", cost: "$2/$6" },
    { id: "mistralai/mistral-small-3.2-24b",        label: "Mistral Small 3.2",       ctx: "128K", cost: "$0.05/$0.10" },
    { id: "mistralai/codestral-2501",               label: "Codestral 2501",          ctx: "256K", cost: "$0.30/$0.90" },
  ]},
  { group: "xAI", models: [
    { id: "x-ai/grok-3",                            label: "Grok 3",                  ctx: "131K", cost: "$3/$15" },
    { id: "x-ai/grok-3-mini",                       label: "Grok 3 Mini",             ctx: "131K", cost: "$0.30/$0.50" },
    { id: "x-ai/grok-2-1212",                       label: "Grok 2",                  ctx: "131K", cost: "$2/$10" },
  ]},
  { group: "Cohere", models: [
    { id: "cohere/command-r-plus-08-2024",          label: "Command R+",              ctx: "128K", cost: "$2.50/$10" },
    { id: "cohere/command-r-08-2024",               label: "Command R",               ctx: "128K", cost: "$0.15/$0.60" },
  ]},
];

// Cost annotations for Claude CLI models — kept for potential future use
// const CLAUDE_MODEL_COSTS: Record<string, string> = { ... };

function claudeModelLabel(_id: string, displayName: string): string {
  return displayName;
}

function OpenRouterSection({
  apiKey, onKeyChange,
}: {
  apiKey: string; onKeyChange: (key: string) => void;
}) {
  const [showKey, setShowKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleTest = async () => {
    if (!apiKey.trim()) return;
    setTesting(true);
    setTestResult(null);
    try {
      const msg = await invoke<string>("test_openrouter_key", { key: apiKey.trim() });
      setTestResult({ ok: true, msg: msg.startsWith("✓") ? msg : "✓ Key valid — OpenRouter connected" });
    } catch (e) {
      setTestResult({ ok: false, msg: String(e) });
    } finally {
      setTesting(false);
      setTimeout(() => setTestResult(null), 6000);
    }
  };

  return (
    <section className="settings-section-block">
      <h4 className="settings-section-title">OpenRouter</h4>
      <div className="settings-section-divider" />
      <span className="settings-field-desc" style={{ display: "block", marginBottom: 10 }}>
        When set, Claude mode bypasses the Copilot proxy and routes directly to OpenRouter.
        Supports GPT-5.4, Gemini, and all other OpenRouter models.
        Get your key at <a href="https://openrouter.ai/keys" target="_blank" rel="noreferrer" style={{ color: "var(--accent-blue)" }}>openrouter.ai/keys</a>.
      </span>
      <div className="settings-or-key-row">
        <input
          type={showKey ? "text" : "password"}
          className="settings-or-key-input"
          placeholder="sk-or-v1-…"
          value={apiKey}
          onChange={(e) => onKeyChange(e.target.value)}
          spellCheck={false}
        />
        <button className="btn-icon-sm" onClick={() => setShowKey((v) => !v)} title={showKey ? "Hide key" : "Show key"}>
          {showKey ? "Hide" : "Show"}
        </button>
        <button className="btn-icon-sm" onClick={handleTest} disabled={testing || !apiKey.trim()}>
          {testing ? "Testing…" : "Test"}
        </button>
      </div>
      {testResult && (
        <span className={testResult.ok ? "settings-auth-success" : "settings-auth-error"} style={{ fontSize: 12, marginTop: 6, display: "block" }}>
          {testResult.msg}
        </span>
      )}
    </section>
  );
}

// ── Main dialog ──────────────────────────────────────────────────

export default function SettingsDialog({ onClose, initialAuthCode, initialAuthUrl }: SettingsDialogProps) {
  const [models, setModels] = useState<ModelEntry[]>([]);
  const [copilotModels, setCopilotModels] = useState<ModelEntry[]>([]);
  const [settings, setSettings] = useState<AppSettings>({
    project_primary_model: "",
    project_secondary_model: "",
    chat_model: "",
    dangerously_skip_permissions: false,
    cli_mode: "claude",
    copilot_model: "gpt-5.4",
    openrouter_api_key: "",
    openrouter_model: "anthropic/claude-sonnet-4-5",
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [pendingCliMode, setPendingCliMode] = useState<"claude" | "copilot" | null>(null);
  const [liveOrModels, setLiveOrModels] = useState<{ id: string; name: string; context_length: number }[] | null>(null);
  const [loadingOrModels, setLoadingOrModels] = useState(false);

  useEffect(() => {
    Promise.all([
      invoke<ModelEntry[]>("get_available_models"),
      invoke<ModelEntry[]>("get_copilot_models"),
      invoke<AppSettings>("get_app_settings"),
    ]).then(([m, cm, s]) => {
      setModels(m);
      setCopilotModels(cm);
      setSettings(s);
      setLoading(false);
      // Fetch live OR models if key is already saved
      if (s.openrouter_api_key.trim()) {
        fetchOrModels(s.openrouter_api_key.trim());
      }
    }).catch((e) => {
      console.error("Failed to load settings:", e);
      setLoading(false);
    });
  }, []);

  const fetchOrModels = useCallback(async (key: string) => {
    if (!key.trim()) { setLiveOrModels(null); return; }
    setLoadingOrModels(true);
    try {
      const list = await invoke<{ id: string; name: string; context_length: number }[]>(
        "get_openrouter_models", { key: key.trim() }
      );
      setLiveOrModels(list);
    } catch {
      setLiveOrModels(null); // fall back to static list
    } finally {
      setLoadingOrModels(false);
    }
  }, []);

  const handleChange = useCallback((key: keyof AppSettings, value: string) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
    setSaved(false);
  }, []);

  const handleSave = useCallback(async () => {
    setSaving(true);
    try {
      await invoke("save_app_settings", { settings });
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      console.error("Failed to save settings:", e);
    } finally {
      setSaving(false);
    }
  }, [settings]);

  const byTier = (tier: string) => models.filter((m) => m.tier === tier);
  const orActive = !!(settings.openrouter_api_key.trim()) && settings.cli_mode === "claude";

  const ModelSelect = ({
    label,
    description,
    settingKey,
    disabled,
  }: {
    label: string;
    description: string;
    settingKey: "project_primary_model" | "project_secondary_model" | "chat_model";
    disabled?: boolean;
  }) => (
    <div className="settings-field">
      <label className="settings-field-label">{label}</label>
      <span className="settings-field-desc">{description}</span>
      <select
        className="settings-select"
        value={settings[settingKey]}
        onChange={(e) => handleChange(settingKey, e.target.value)}
        disabled={loading || disabled}
      >
        {loading && <option value="">Loading models…</option>}
        {(["opus", "sonnet", "haiku"] as const).map((tier) => {
          const tier_models = byTier(tier);
          if (tier_models.length === 0) return null;
          return (
            <optgroup key={tier} label={TIER_LABELS[tier]}>
              {tier_models.map((m) => (
                <option key={m.id} value={m.id}>
                  {claudeModelLabel(m.id, m.display_name)}
                </option>
              ))}
            </optgroup>
          );
        })}
      </select>
    </div>
  );

  return (
    <>
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="settings-dialog-header">
          <h3>⚙ Settings</h3>
          <button className="settings-close-btn" onClick={onClose} aria-label="Close settings">
            ✕
          </button>
        </div>

        <div className="settings-dialog-body">

          {/* ── Account ─────────────────────────────────── */}
          <AccountSection
            initialAuthCode={initialAuthCode}
            initialAuthUrl={initialAuthUrl}
            openrouterActive={!!(settings.openrouter_api_key.trim()) && settings.cli_mode === "claude"}
          />

          {/* ── OpenRouter ──────────────────────────────── */}
          <OpenRouterSection
            apiKey={settings.openrouter_api_key}
            onKeyChange={(key) => { setSettings((prev) => ({ ...prev, openrouter_api_key: key })); setSaved(false); }}
          />

          {/* ── Terminal ────────────────────────────────── */}
          <section className="settings-section-block">
            <h4 className="settings-section-title">Terminal</h4>
            <div className="settings-section-divider" />

            {/* CLI Mode toggle */}
            <div className="settings-field">
              <span className="settings-field-label">CLI Mode</span>
              <span className="settings-field-desc">
                Choose how the terminal launches. Switching mode will close all open tabs.
              </span>
              <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
                <button
                  className={`cli-mode-btn${settings.cli_mode === "claude" ? " cli-mode-btn--active" : ""}`}
                  onClick={() => { if (settings.cli_mode !== "claude") setPendingCliMode("claude"); }}
                  disabled={loading}
                >
                  Claude CLI + Copilot Proxy
                </button>
                <button
                  className={`cli-mode-btn${settings.cli_mode === "copilot" ? " cli-mode-btn--active" : ""}`}
                  onClick={() => { if (settings.cli_mode !== "copilot") setPendingCliMode("copilot"); }}
                  disabled={loading}
                >
                  Copilot CLI
                </button>
              </div>
            </div>

            <div className={`settings-field${settings.cli_mode === "copilot" ? " settings-field--disabled" : ""}`} style={{ marginTop: 16 }}>
              <div className="settings-toggle-row">
                <label className="settings-toggle-label">
                  <span className="settings-field-label">
                    Skip permission prompts
                    {settings.cli_mode === "copilot" && (
                      <span className="settings-field-note"> — Claude only</span>
                    )}
                  </span>
                  <label className="settings-toggle">
                    <input
                      type="checkbox"
                      checked={settings.dangerously_skip_permissions}
                      disabled={settings.cli_mode === "copilot"}
                      onChange={(e) => {
                        setSettings((prev) => ({ ...prev, dangerously_skip_permissions: e.target.checked }));
                        setSaved(false);
                      }}
                    />
                    <span className="settings-toggle-slider" />
                  </label>
                </label>
              </div>
              <div className="settings-warning">
                <span className="settings-warning-icon">&#9888;</span>
                <span>
                  {settings.cli_mode === "copilot"
                    ? <>Not applicable in Copilot mode. Switch to Claude mode to use this setting.</>
                    : <>Launches Claude with <code>--dangerously-skip-permissions</code>. This allows Claude to execute commands, edit files, and access the internet <strong>without asking for confirmation</strong>. Only enable this if you fully understand the risks.</>
                  }
                </span>
              </div>
            </div>
          </section>

          {/* ── Models ──────────────────────────────────── */}
          <section className="settings-section-block">
            <h4 className="settings-section-title">Models</h4>
            <div className="settings-section-divider" />

            {/* Active mode block renders first, inactive below dimmed */}
            {(() => {
              const isClaudeDisabled = settings.cli_mode === "copilot" || orActive;
              const claudeDisabledReason = settings.cli_mode === "copilot"
                ? "Claude model selection is not used in Copilot mode."
                : orActive
                ? "Claude model selection is overridden by OpenRouter — set the model in the OpenRouter block below."
                : "";

              const claudeBlock = (
                <div className={isClaudeDisabled ? "settings-field--disabled" : ""}>
                  <div className="settings-model-note">
                    {isClaudeDisabled
                      ? claudeDisabledReason
                      : "Model selection is applied when a new tab is opened. Models are loaded live from the Claude CLI — newest versions appear automatically."
                    }
                  </div>
                  <div className="settings-fields">
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
                      <div className="settings-field-group-label" style={{ margin: 0 }}>Projects</div>
                      <button
                        className="btn-icon-sm"
                        title="Refresh model list from Claude CLI"
                        onClick={async () => {
                          try {
                            const refreshed = await invoke<ModelEntry[]>("get_available_models");
                            if (refreshed.length > 0) setModels(refreshed);
                          } catch (e) { console.warn("get_available_models failed:", e); }
                        }}
                      >⟳ Refresh</button>
                    </div>
                    <ModelSelect label="Primary model" description="Used for deep thinking, planning, and architecture — launched when you open a project." settingKey="project_primary_model" disabled={isClaudeDisabled} />
                    <ModelSelect label="Fallback model" description="Automatically used when the primary model is overloaded." settingKey="project_secondary_model" disabled={isClaudeDisabled} />
                    <div className="settings-field-group-label" style={{ marginTop: 16 }}>Chats</div>
                    <ModelSelect label="Chat model" description="Used for quick conversations and scratch tabs." settingKey="chat_model" disabled={isClaudeDisabled} />
                  </div>
                </div>
              );

              const copilotBlock = (
                <div className={settings.cli_mode === "claude" ? "settings-field--disabled" : ""} style={{ marginTop: 16 }}>
                  <div className="settings-model-note">
                    {settings.cli_mode === "claude"
                      ? "Copilot model selection is not used in Claude mode."
                      : "Passed as --model when launching the Copilot CLI."
                    }
                  </div>
                  <div className="settings-fields">
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
                      <div className="settings-field-group-label" style={{ margin: 0 }}>Copilot</div>
                      <button
                        className="btn-icon-sm"
                        title="Scan installed Copilot CLI for latest models"
                        onClick={async () => {
                          try {
                            const scanned = await invoke<ModelEntry[]>("scan_copilot_models");
                            if (scanned.length > 0) setCopilotModels(scanned);
                          } catch (e) { console.warn("scan_copilot_models failed:", e); }
                        }}
                      >⟳ Refresh</button>
                    </div>
                    <div className="settings-field">
                      <label className="settings-field-label">Model</label>
                      <select
                        className="settings-select"
                        value={settings.copilot_model}
                        disabled={loading || settings.cli_mode === "claude"}
                        onChange={(e) => {
                          setSettings((prev) => ({ ...prev, copilot_model: e.target.value }));
                          setSaved(false);
                        }}
                      >
                        {loading && <option value="">Loading…</option>}
                        {copilotModels.map((m) => (
                          <option key={m.id} value={m.id}>{m.display_name}</option>
                        ))}
                      </select>
                    </div>
                  </div>
                </div>
              );

              const orBlock = (() => {
                // Well-known provider ID → display label overrides.
                // Without these, generic title-casing produces awkward results
                // like "X Ai" (for "x-ai") or "Mistralai" (for "mistralai").
                const PROVIDER_LABELS: Record<string, string> = {
                  "x-ai": "xAI",
                  "mistralai": "Mistral AI",
                  "deepseek": "DeepSeek",
                  "perplexity": "Perplexity",
                  "cohere": "Cohere",
                  "nousresearch": "Nous Research",
                  "sao10k": "Sao10k",
                  "openrouter": "OpenRouter",
                };
                const formatProvider = (raw: string): string =>
                  PROVIDER_LABELS[raw] ??
                  raw.replace(/-/g, " ").replace(/\b\w/g, c => c.toUpperCase());

                // Build grouped options: live from API, or fall back to static list
                const orModelOptions = liveOrModels
                  ? (() => {
                      // Group by provider prefix (e.g. "anthropic/..." → "Anthropic")
                      const grouped: Record<string, { id: string; name: string; context_length: number }[]> = {};
                      for (const m of liveOrModels) {
                        const provider = m.id.includes("/")
                          ? formatProvider(m.id.split("/")[0])
                          : "Other";
                        if (!grouped[provider]) grouped[provider] = [];
                        grouped[provider].push(m);
                      }
                      return Object.entries(grouped).map(([group, models]) => ({
                        group,
                        models: models.map(m => ({
                          id: m.id,
                          label: m.name,
                          ctx: m.context_length >= 1_000_000
                            ? `${(m.context_length / 1_000_000).toFixed(0)}M`
                            : m.context_length >= 1000
                            ? `${Math.round(m.context_length / 1000)}K`
                            : `${m.context_length}`,
                        })),
                      }));
                    })()
                  : OR_MODELS.map(g => ({
                      group: g.group,
                      models: g.models.map(m => ({ id: m.id, label: m.label, ctx: m.ctx })),
                    }));

                const currentInList = orModelOptions.flatMap(g => g.models).some(m => m.id === settings.openrouter_model);

                return (
                  <div className={!orActive ? "settings-field--disabled" : ""} style={{ marginTop: 16 }}>
                    <div className="settings-model-note">
                      {orActive
                        ? "OpenRouter is active. This model is passed as --model when launching Claude Code."
                        : "Set an OpenRouter API key above to enable this section."
                      }
                    </div>
                    <div className="settings-fields">
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 4 }}>
                        <div className="settings-field-group-label" style={{ margin: 0 }}>OpenRouter</div>
                        {orActive && (
                          <button
                            className="btn-icon-sm"
                            title="Reload model list from OpenRouter API"
                            disabled={loadingOrModels}
                            onClick={() => fetchOrModels(settings.openrouter_api_key)}
                          >{loadingOrModels ? "Loading…" : "⟳ Refresh"}</button>
                        )}
                      </div>
                      <div className="settings-field">
                        <label className="settings-field-label">Model</label>
                        <span className="settings-field-desc">
                          {liveOrModels
                            ? `${liveOrModels.length} models loaded live from OpenRouter.`
                            : "Showing built-in list. Save a valid API key and click ⟳ Refresh to load all available models."
                          }
                        </span>
                        <select
                          className="settings-select"
                          value={settings.openrouter_model}
                          disabled={!orActive}
                          onChange={(e) => { setSettings((prev) => ({ ...prev, openrouter_model: e.target.value })); setSaved(false); }}
                        >
                          {orModelOptions.map((group) => (
                            <optgroup key={group.group} label={group.group}>
                              {group.models.map((m) => (
                                <option key={m.id} value={m.id}>
                                  {m.label}  ·  {m.ctx} ctx
                                </option>
                              ))}
                            </optgroup>
                          ))}
                          {!currentInList && settings.openrouter_model && (
                            <option value={settings.openrouter_model}>{settings.openrouter_model} (custom)</option>
                          )}
                        </select>
                      </div>
                    </div>
                  </div>
                );
              })();

              if (settings.cli_mode === "copilot") {
                return <>{copilotBlock}{claudeBlock}{orBlock}</>;
              }
              if (orActive) {
                return <>{orBlock}{claudeBlock}{copilotBlock}</>;
              }
              return <>{claudeBlock}{copilotBlock}{orBlock}</>;
            })()}

          </section>

        </div>

        {/* ── Sticky footer ───────────────────────────── */}
        <div className="settings-dialog-footer">
          <button
            className={`btn btn-start${saved ? " btn-saved" : ""}`}
            onClick={handleSave}
            disabled={saving || loading}
          >
            {saving ? "Saving…" : saved ? "✓ Saved" : "Save"}
          </button>
        </div>
      </div>
    </div>

    {pendingCliMode && (
      <CliModeWarningModal
        targetMode={pendingCliMode}
        projectPath={null}
        onCancel={() => setPendingCliMode(null)}
        onConfirm={async (syncFiles) => {
          const newMode = pendingCliMode;
          setPendingCliMode(null);
          setSettings((prev) => ({ ...prev, cli_mode: newMode }));
          setSaved(false);
          try {
            await invoke("save_app_settings", {
              settings: { ...settings, cli_mode: newMode },
            });
            await invoke("apply_cli_mode", { mode: newMode, syncFiles });
          } catch (e) {
            console.error("Failed to apply CLI mode:", e);
          }
        }}
      />
    )}
    </>
  );
}

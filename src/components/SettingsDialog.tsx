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
}

const TIER_LABELS: Record<string, string> = {
  opus:   "Opus — deep reasoning & planning",
  sonnet: "Sonnet — coding & general work",
  haiku:  "Haiku — fast & lightweight",
};

// ── Account section ──────────────────────────────────────────────

function AccountSection({ initialAuthCode, initialAuthUrl }: { initialAuthCode?: string | null; initialAuthUrl?: string | null }) {
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
      const codeMatch = line.match(/"([A-Z0-9]{4}-[A-Z0-9]{4})"/);
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
      <h4 className="settings-section-title">Account</h4>
      <div className="settings-section-divider" />

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
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [pendingCliMode, setPendingCliMode] = useState<"claude" | "copilot" | null>(null);

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
    }).catch((e) => {
      console.error("Failed to load settings:", e);
      setLoading(false);
    });
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

  const ModelSelect = ({
    label,
    description,
    settingKey,
  }: {
    label: string;
    description: string;
    settingKey: "project_primary_model" | "project_secondary_model" | "chat_model";
  }) => (
    <div className="settings-field">
      <label className="settings-field-label">{label}</label>
      <span className="settings-field-desc">{description}</span>
      <select
        className="settings-select"
        value={settings[settingKey]}
        onChange={(e) => handleChange(settingKey, e.target.value)}
        disabled={loading}
      >
        {loading && <option value="">Loading models…</option>}
        {(["opus", "sonnet", "haiku"] as const).map((tier) => {
          const tier_models = byTier(tier);
          if (tier_models.length === 0) return null;
          return (
            <optgroup key={tier} label={TIER_LABELS[tier]}>
              {tier_models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.display_name}
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
          <AccountSection initialAuthCode={initialAuthCode} initialAuthUrl={initialAuthUrl} />

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
              const claudeBlock = (
                <div className={settings.cli_mode === "copilot" ? "settings-field--disabled" : ""}>
                  <div className="settings-model-note">
                    {settings.cli_mode === "copilot"
                      ? "Claude model selection is not used in Copilot mode."
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
                    <ModelSelect label="Primary model" description="Used for deep thinking, planning, and architecture — launched when you open a project." settingKey="project_primary_model" />
                    <ModelSelect label="Fallback model" description="Automatically used when the primary model is overloaded." settingKey="project_secondary_model" />
                    <div className="settings-field-group-label" style={{ marginTop: 16 }}>Chats</div>
                    <ModelSelect label="Chat model" description="Used for quick conversations and scratch tabs." settingKey="chat_model" />
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

              return settings.cli_mode === "copilot"
                ? <>{copilotBlock}{claudeBlock}</>
                : <>{claudeBlock}{copilotBlock}</>;
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

import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

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
  const [settings, setSettings] = useState<AppSettings>({
    project_primary_model: "",
    project_secondary_model: "",
    chat_model: "",
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    Promise.all([
      invoke<ModelEntry[]>("get_available_models"),
      invoke<AppSettings>("get_app_settings"),
    ]).then(([m, s]) => {
      setModels(m);
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
    settingKey: keyof AppSettings;
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
            <p className="settings-section-empty">Terminal settings coming soon.</p>
          </section>

          {/* ── Models ──────────────────────────────────── */}
          <section className="settings-section-block">
            <h4 className="settings-section-title">Models</h4>
            <div className="settings-section-divider" />

            <div className="settings-model-note">
              Model selection is applied when a new tab is opened. Models are loaded
              live from the Claude CLI — newest versions appear automatically.
            </div>

            <div className="settings-fields">
              <div className="settings-field-group-label">Projects</div>

              <ModelSelect
                label="Primary model"
                description="Used for deep thinking, planning, and architecture — launched when you open a project."
                settingKey="project_primary_model"
              />

              <ModelSelect
                label="Fallback model"
                description="Automatically used when the primary model is overloaded."
                settingKey="project_secondary_model"
              />

              <div className="settings-field-group-label" style={{ marginTop: 16 }}>Chats</div>

              <ModelSelect
                label="Chat model"
                description="Used for quick conversations and scratch tabs."
                settingKey="chat_model"
              />
            </div>

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
  );
}

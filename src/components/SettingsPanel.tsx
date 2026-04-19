import React from "react";
import { themes, applyTheme, getSavedTheme } from "../themes";

export const SettingsPanel: React.FC = () => {
  const [activeTheme, setActiveTheme] = React.useState(getSavedTheme);

  const handleThemeSelect = (themeName: string) => {
    applyTheme(themeName);
    setActiveTheme(themeName);
  };

  const darkThemes = themes.filter((t) => !t.light);
  const lightThemes = themes.filter((t) => t.light);

  return (
    <div className="side-panel-content">
      <h3>Theme</h3>

      <div className="settings-section">
        <label className="settings-section-label">Dark Themes</label>
        <div className="theme-grid">
          {darkThemes.map((theme) => (
            <button
              key={theme.name}
              className={`theme-card${activeTheme === theme.name ? " active" : ""}`}
              onClick={() => handleThemeSelect(theme.name)}
            >
              <div className="theme-swatches">
                <span className="swatch" style={{ background: theme.colors["--bg-primary"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-blue"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-green"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-purple"] }} />
              </div>
              <span className="theme-card-label">{theme.label}</span>
            </button>
          ))}
        </div>
      </div>

      <div className="settings-section">
        <label className="settings-section-label">Light Themes</label>
        <div className="theme-grid">
          {lightThemes.map((theme) => (
            <button
              key={theme.name}
              className={`theme-card${activeTheme === theme.name ? " active" : ""}`}
              onClick={() => handleThemeSelect(theme.name)}
            >
              <div className="theme-swatches">
                <span className="swatch" style={{ background: theme.colors["--bg-primary"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-blue"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-green"] }} />
                <span className="swatch" style={{ background: theme.colors["--accent-purple"] }} />
              </div>
              <span className="theme-card-label">{theme.label}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
};

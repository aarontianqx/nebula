import { useState, useEffect } from "react";
import { X, Palette, Database, RefreshCw, CheckCircle, AlertCircle, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

interface UserSettings {
  theme: string | null;
  storage: {
    type: "sqlite" | "mongodb";
    mongodb: {
      uri: string;
      database: string;
    };
  };
}

interface SettingsResponse {
  settings: UserSettings;
  availableThemes: string[];
  defaultTheme: string;
}

interface ThemeResponse {
  activeTheme: string;
  cssVars: Record<string, string>;
  availableThemes: string[];
}

// Apply theme by injecting CSS variables into document root
const applyTheme = (cssVars: Record<string, string>) => {
  for (const [key, value] of Object.entries(cssVars)) {
    document.documentElement.style.setProperty(key, value);
  }
};

interface Props {
  onClose: () => void;
  onThemeChange?: () => void;
}

function SettingsDialog({ onClose, onThemeChange }: Props) {
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [originalSettings, setOriginalSettings] = useState<UserSettings | null>(null);
  const [originalCssVars, setOriginalCssVars] = useState<Record<string, string> | null>(null);
  const [availableThemes, setAvailableThemes] = useState<string[]>([]);
  const [defaultTheme, setDefaultTheme] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  // MongoDB connection test state
  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionTestResult, setConnectionTestResult] = useState<{
    success: boolean;
    message: string;
  } | null>(null);

  // Load settings on mount and capture original theme for rollback
  useEffect(() => {
    loadSettings();
  }, []);

  // Reset connection test result when MongoDB config changes
  useEffect(() => {
    setConnectionTestResult(null);
  }, [settings?.storage.mongodb.uri, settings?.storage.mongodb.database]);

  const loadSettings = async () => {
    try {
      const response: SettingsResponse = await invoke("get_settings");
      setSettings(response.settings);
      setOriginalSettings(response.settings);
      setAvailableThemes(response.availableThemes);
      setDefaultTheme(response.defaultTheme);
      setError(null);

      // Capture current CSS vars for rollback on cancel
      const themeConfig: ThemeResponse = await invoke("get_theme_config");
      setOriginalCssVars(themeConfig.cssVars);
    } catch (e) {
      setError(`Failed to load settings: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const testMongoConnection = async () => {
    if (!settings) return;

    setTestingConnection(true);
    setConnectionTestResult(null);

    try {
      await invoke("test_mongodb_connection", {
        uri: settings.storage.mongodb.uri,
        database: settings.storage.mongodb.database,
      });
      setConnectionTestResult({
        success: true,
        message: "Connection successful!",
      });
    } catch (e) {
      setConnectionTestResult({
        success: false,
        message: String(e),
      });
    } finally {
      setTestingConnection(false);
    }
  };

  const handleSave = async () => {
    if (!settings) return;

    // If MongoDB is selected, test connection before saving
    if (settings.storage.type === "mongodb") {
      setTestingConnection(true);
      setError(null);

      try {
        await invoke("test_mongodb_connection", {
          uri: settings.storage.mongodb.uri,
          database: settings.storage.mongodb.database,
        });
      } catch (e) {
        setTestingConnection(false);
        setError(`Cannot save: MongoDB connection failed.\n\n${e}`);
        setConnectionTestResult({
          success: false,
          message: String(e),
        });
        return;
      }

      setTestingConnection(false);
      setConnectionTestResult({
        success: true,
        message: "Connection successful!",
      });
    }

    setSaving(true);
    try {
      await invoke("save_settings", { settings });

      // Apply theme immediately
      const themeConfig: ThemeResponse = await invoke("get_theme_config");
      applyTheme(themeConfig.cssVars);

      onThemeChange?.();
      onClose();
    } catch (e) {
      setError(`Failed to save settings: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handleThemeChange = async (themeName: string) => {
    if (!settings) return;

    // Update settings state (local only, not saved yet)
    setSettings({
      ...settings,
      theme: themeName === defaultTheme ? null : themeName,
    });

    // Apply theme immediately for live preview
    // We temporarily save to get the resolved theme, then restore on cancel if needed
    try {
      const tempSettings = {
        ...settings,
        theme: themeName === defaultTheme ? null : themeName,
      };
      await invoke("save_settings", { settings: tempSettings });

      const themeConfig: ThemeResponse = await invoke("get_theme_config");
      applyTheme(themeConfig.cssVars);
    } catch (e) {
      console.error("Failed to preview theme:", e);
    }
  };

  // Handle cancel: restore original theme
  const handleCancel = async () => {
    // Restore original settings to disk
    if (originalSettings) {
      try {
        await invoke("save_settings", { settings: originalSettings });
      } catch (e) {
        console.error("Failed to restore settings:", e);
      }
    }

    // Restore original CSS vars
    if (originalCssVars) {
      applyTheme(originalCssVars);
    }

    onClose();
  };

  const handleStorageTypeChange = (type: "sqlite" | "mongodb") => {
    if (!settings) return;
    setSettings({
      ...settings,
      storage: { ...settings.storage, type },
    });
    setError(null);
    setConnectionTestResult(null);
  };

  const handleMongoUriChange = (uri: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      storage: {
        ...settings.storage,
        mongodb: { ...settings.storage.mongodb, uri },
      },
    });
  };

  const handleMongoDatabaseChange = (database: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      storage: {
        ...settings.storage,
        mongodb: { ...settings.storage.mongodb, database },
      },
    });
  };

  const currentTheme = settings?.theme || defaultTheme;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-[var(--color-bg-panel)] rounded-lg w-[500px] max-h-[80vh] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
          <h2 className="text-lg font-semibold">Settings</h2>
          <button
            onClick={handleCancel}
            className="p-1 rounded hover:bg-[var(--color-bg-hover)] transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-6">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="animate-spin text-[var(--color-text-muted)]" size={24} />
            </div>
          ) : (
            <>
              {/* Error Banner */}
              {error && (
                <div className="p-3 bg-[var(--color-error)]/20 text-[var(--color-error)] rounded-lg text-sm whitespace-pre-wrap">
                  {error}
                </div>
              )}

              {/* Theme Section */}
              <section>
                <div className="flex items-center gap-2 mb-3">
                  <Palette size={18} className="text-[var(--color-accent)]" />
                  <h3 className="font-medium">Theme</h3>
                </div>
                <div className="grid grid-cols-2 gap-2">
                  {availableThemes.map((theme) => (
                    <button
                      key={theme}
                      onClick={() => handleThemeChange(theme)}
                      className={`px-3 py-2 rounded-lg text-sm text-left transition-colors ${currentTheme === theme
                        ? "bg-[var(--color-accent)] text-[var(--color-accent-fg)]"
                        : "bg-[var(--color-bg-surface)] hover:bg-[var(--color-bg-hover)]"
                        }`}
                    >
                      {theme}
                      {theme === defaultTheme && (
                        <span className="text-xs opacity-70 ml-1">(default)</span>
                      )}
                    </button>
                  ))}
                </div>
                <p className="text-xs text-[var(--color-text-muted)] mt-2">
                  Click to preview. Press Save to keep changes.
                </p>
              </section>

              {/* Storage Section */}
              <section>
                <div className="flex items-center gap-2 mb-3">
                  <Database size={18} className="text-[var(--color-accent)]" />
                  <h3 className="font-medium">Storage</h3>
                </div>

                <div className="space-y-3">
                  {/* Storage Type */}
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleStorageTypeChange("sqlite")}
                      className={`flex-1 px-3 py-2 rounded-lg text-sm transition-colors ${settings?.storage.type === "sqlite"
                        ? "bg-[var(--color-accent)] text-[var(--color-accent-fg)]"
                        : "bg-[var(--color-bg-surface)] hover:bg-[var(--color-bg-hover)]"
                        }`}
                    >
                      SQLite (Local)
                    </button>
                    <button
                      onClick={() => handleStorageTypeChange("mongodb")}
                      className={`flex-1 px-3 py-2 rounded-lg text-sm transition-colors ${settings?.storage.type === "mongodb"
                        ? "bg-[var(--color-accent)] text-[var(--color-accent-fg)]"
                        : "bg-[var(--color-bg-surface)] hover:bg-[var(--color-bg-hover)]"
                        }`}
                    >
                      MongoDB (Remote)
                    </button>
                  </div>

                  {/* MongoDB Config */}
                  {settings?.storage.type === "mongodb" && (
                    <div className="space-y-3 p-3 bg-[var(--color-bg-surface)] rounded-lg">
                      <div>
                        <label className="block text-xs text-[var(--color-text-secondary)] mb-1">
                          Connection URI
                        </label>
                        <input
                          type="text"
                          value={settings.storage.mongodb.uri}
                          onChange={(e) => handleMongoUriChange(e.target.value)}
                          placeholder="mongodb://localhost:27017"
                          className="w-full px-3 py-2 bg-[var(--color-bg-app)] border border-[var(--color-border)] rounded text-sm focus:outline-none focus:border-[var(--color-accent)]"
                        />
                      </div>
                      <div>
                        <label className="block text-xs text-[var(--color-text-secondary)] mb-1">
                          Database Name
                        </label>
                        <input
                          type="text"
                          value={settings.storage.mongodb.database}
                          onChange={(e) => handleMongoDatabaseChange(e.target.value)}
                          placeholder="wardenly"
                          className="w-full px-3 py-2 bg-[var(--color-bg-app)] border border-[var(--color-border)] rounded text-sm focus:outline-none focus:border-[var(--color-accent)]"
                        />
                      </div>

                      {/* Test Connection Button & Result */}
                      <div className="space-y-2">
                        <button
                          onClick={testMongoConnection}
                          disabled={testingConnection || !settings.storage.mongodb.uri}
                          className="px-3 py-1.5 text-xs bg-[var(--color-bg-app)] border border-[var(--color-border)] rounded hover:bg-[var(--color-bg-hover)] transition-colors disabled:opacity-50 flex items-center gap-1.5"
                        >
                          {testingConnection ? (
                            <>
                              <Loader2 size={14} className="animate-spin" />
                              Testing...
                            </>
                          ) : (
                            "Test Connection"
                          )}
                        </button>

                        {/* Connection Test Result - full width for better readability */}
                        {connectionTestResult && (
                          <div className={`flex items-start gap-1.5 text-xs p-2 rounded ${
                            connectionTestResult.success 
                              ? "text-[var(--color-success)] bg-[var(--color-success)]/10" 
                              : "text-[var(--color-error)] bg-[var(--color-error)]/10"
                          }`}>
                            {connectionTestResult.success ? (
                              <CheckCircle size={14} className="flex-shrink-0 mt-0.5" />
                            ) : (
                              <AlertCircle size={14} className="flex-shrink-0 mt-0.5" />
                            )}
                            <span className="break-all">
                              {connectionTestResult.message}
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  <p className="text-xs text-[var(--color-text-muted)]">
                    {settings?.storage.type === "mongodb" 
                      ? "Connection will be verified before saving. Restart required to apply."
                      : "Storage changes require restarting the application."}
                  </p>
                </div>
              </section>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-[var(--color-border)]">
          <button
            onClick={handleCancel}
            className="px-4 py-2 text-sm rounded-lg hover:bg-[var(--color-bg-hover)] transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving || loading || testingConnection}
            className="px-4 py-2 text-sm bg-[var(--color-accent)] text-[var(--color-accent-fg)] rounded-lg hover:bg-[var(--color-accent-hover)] transition-colors disabled:opacity-50 flex items-center gap-2"
          >
            {(saving || testingConnection) && <Loader2 size={14} className="animate-spin" />}
            {testingConnection ? "Verifying..." : saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SettingsDialog;



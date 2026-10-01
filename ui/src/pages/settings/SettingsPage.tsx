import { useEffect, useState, type ReactNode } from "react";
import type { AppConfig, SecurityMode } from "@protocol/v1";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";

const emptyConfig: AppConfig = {
  schemaVersion: 1,
  configRevision: "0",
  workspace: { rootPath: "" },
  tunnel: { tunnelId: "", autoDetect: true },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
  tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
};

function validateControlPlaneProxy(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "";
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return "Enter a valid HTTP or HTTPS proxy URL.";
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return "Proxy URL must use http:// or https://.";
  if (!parsed.hostname) return "Proxy URL must include a host.";
  if (parsed.username || parsed.password) return "Proxy credentials are not supported.";
  if (parsed.pathname !== "/" || parsed.search || parsed.hash) return "Proxy URL cannot include a path, query, or fragment.";
  return "";
}

export function SettingsPage() {
  const data = useHarnessData();
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [replacingApiKey, setReplacingApiKey] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [proxyError, setProxyError] = useState("");
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (!dirty) setForm(data.config ?? emptyConfig);
  }, [data.config, dirty]);

  const update = (patch: Partial<AppConfig>) => {
    setForm((current) => ({ ...current, ...patch }));
    setDirty(true);
    setMessage("");
    setError("");
  };

  const chooseFolder = async () => {
    const path = await data.chooseWorkspace();
    if (path) update({ workspace: { rootPath: path } });
  };

  const chooseClient = async () => {
    const path = await data.chooseTunnelClient();
    if (path) update({ tunnel: { ...form.tunnel, tunnelClientPath: path, autoDetect: false } });
  };

  const detect = async () => {
    const path = await data.detectTunnelClient();
    if (path) update({ tunnel: { ...form.tunnel, tunnelClientPath: path, autoDetect: true } });
    else setMessage("The bundled Tunnel client will be used in the installed build.");
  };

  const setMode = (mode: SecurityMode) => {
    update({ security: { mode, developerTestMode: mode === "developer_test" } });
  };

  const save = async () => {
    setError("");
    if (!form.workspace.rootPath.trim()) return setError("Choose a workspace folder.");
    const nextProxy = form.tunnel.controlPlaneHttpProxy?.trim() ?? "";
    const nextProxyError = validateControlPlaneProxy(nextProxy);
    setProxyError(nextProxyError);
    if (nextProxyError) return;
    setSaving(true);
    const result = await data.saveConfig(
      {
        ...form,
        tunnel: {
          ...form.tunnel,
          tunnelId: form.tunnel.tunnelId.trim(),
          controlPlaneHttpProxy: nextProxy || undefined,
        },
      },
      apiKey.trim() || undefined,
    );
    setSaving(false);
    setApiKey("");
    if (!result.ok) return setError(result.error?.message ?? "Could not save configuration.");

    setReplacingApiKey(false);
    setDirty(false);
    setMessage("Changes saved.");
    await data.refreshConfig();
  };

  const reset = async () => {
    if (!window.confirm("Reset local configuration and remove the stored API Key?")) return;
    setResetting(true);
    const result = await data.resetConfig();
    setResetting(false);
    if (!result.ok) return setError(result.error?.message ?? "Could not reset configuration.");
    setForm(emptyConfig);
    setApiKey("");
    setReplacingApiKey(false);
    setDirty(false);
    setMessage("Configuration reset. The next save will start setup again.");
  };

  const restartHarness = async () => {
    setError("");
    const result = await data.restartHarness();
    if (!result.ok) return setError(result.error?.message ?? "Could not restart Harness.");
    setMessage("Harness restarted with the saved configuration.");
  };

  return (
    <div className="settings-page page-stack">
      <Card className="settings-banner">
        <div className="settings-lock">⌁</div>
        <div>
          <StatusBadge tone="success">M2 controls enabled</StatusBadge>
          <h2>Configuration stays local and explicit</h2>
          <p>Manage the local control-plane settings without changing the application shell.</p>
        </div>
      </Card>

      {data.configError && (
        <div className="notice notice-danger">
          <strong>Configuration needs attention</strong>
          <span>{data.configError.message}</span>
          <button className="button button-quiet" onClick={() => void reset()} disabled={resetting}>Reset configuration</button>
        </div>
      )}
      {data.restartRequired && (
        <div className="notice notice-warning">
          <strong>Config Mismatch — Restart Required</strong>
          <span>The saved configuration differs from the active Harness session.</span>
          <button className="button button-warning" onClick={() => void restartHarness()}>Restart Harness</button>
        </div>
      )}

      <div className="settings-layout">
        <SettingsSection eyebrow="Workspace" title="Active workspace">
          <div className="form-row">
            <label className="form-field grow">
              <span>Workspace root</span>
              <input
                className="path-input"
                value={form.workspace.rootPath} readOnly
                placeholder="Choose a folder"
                title={form.workspace.rootPath || undefined}
              />
            </label>
            <button className="button button-secondary align-end" onClick={() => void chooseFolder()}>Choose Folder</button>
          </div>
          <p className="settings-help">Only this workspace will be used by the Harness.</p>
        </SettingsSection>

        <SettingsSection eyebrow="Tunnel connection" title="Required for Harness">
          <label className="form-field">
            <span>Tunnel ID (required)</span>
            <input value={form.tunnel.tunnelId} onChange={(event) => update({ tunnel: { ...form.tunnel, tunnelId: event.target.value } })} placeholder="Enter tunnel ID" />
          </label>
          <label className="form-field">
            <span>Control-plane HTTP proxy (optional)</span>
            <input
              value={form.tunnel.controlPlaneHttpProxy ?? ""}
              onChange={(event) => {
                setProxyError("");
                update({ tunnel: { ...form.tunnel, controlPlaneHttpProxy: event.target.value || undefined } });
              }}
              placeholder="http://127.0.0.1:7897"
              aria-invalid={Boolean(proxyError)}
              aria-describedby={proxyError ? "control-plane-proxy-error" : undefined}
            />
            {proxyError && <span id="control-plane-proxy-error" className="settings-help danger-text">{proxyError}</span>}
          </label>
          <p className="settings-help">Only OpenAI control-plane traffic uses this proxy. Local MCP remains direct. Leave blank to allow the CONTROL_PLANE_HTTP_PROXY environment variable.</p>
          <div className="form-row">
            <label className="form-field grow">
              <span>Development Tunnel override (optional)</span>
              <input className="path-input" value={form.tunnel.tunnelClientPath ?? ""} readOnly placeholder="Not selected" title={form.tunnel.tunnelClientPath || undefined} />
            </label>
            <button className="button button-secondary align-end" onClick={() => void detect()}>Use Bundled</button>
            <button className="button button-secondary align-end" onClick={() => void chooseClient()}>Choose Override</button>
          </div>
          <label className="form-field">
            <span>API Key (required)</span>
            {data.apiKeyConfigured && !replacingApiKey ? (
              <div className="secret-state">
                <StatusBadge tone="success">Configured</StatusBadge>
                <button className="button button-quiet" onClick={() => { setReplacingApiKey(true); setApiKey(""); }}>Replace</button>
                <button className="button button-quiet danger-text" onClick={() => { setReplacingApiKey(false); setApiKey(""); void data.deleteApiKey(); }}>Remove</button>
              </div>
            ) : (
              <input type="password" value={apiKey} onChange={(event) => { setApiKey(event.target.value); setDirty(true); setError(""); }} placeholder="Enter API Key" autoComplete="off" />
            )}
          </label>
        </SettingsSection>

        <SettingsSection eyebrow="Security" title="Mode">
          <div className="mode-options">
            <button className={`mode-option ${form.security.mode === "normal" ? "selected" : ""}`} onClick={() => setMode("normal")}>
              <strong>Normal Mode</strong>
              <span>Recommended · Read-only default capabilities.</span>
            </button>
            <button className={`mode-option developer ${form.security.mode === "developer_test" ? "selected" : ""}`} onClick={() => setMode("developer_test")}>
              <strong>Developer/Test Mode</strong>
              <span>Higher-risk developer capabilities may become available in later milestones.</span>
            </button>
          </div>
        </SettingsSection>

        <SettingsSection eyebrow="Local data" title="Storage boundaries">
          <div className="local-facts">
            <div><span>Config directory</span><code className="path-value" title={data.configMetadata?.configDirectory}>{data.configMetadata?.configDirectory ?? "OS application data"}</code></div>
            <div><span>Config file</span><code className="path-value" title={data.configMetadata?.configFile}>{data.configMetadata?.configFile ?? "config.json"}</code></div>
            <div><span>Secret provider</span><code>{data.config?.tunnel.apiKeyRef?.provider ?? "OS credential store"}</code></div>
          </div>
        </SettingsSection>
      </div>

      {error && <div className="notice notice-danger">{error}</div>}
      {message && <div className="notice notice-success">{message}</div>}
      <div className="settings-actions">
        <span className={dirty ? "warning-text" : "muted"}>{dirty ? "Unsaved changes" : "All changes saved"}</span>
        <button className="button button-secondary" disabled={!dirty || saving} onClick={() => { setForm(data.config ?? emptyConfig); setApiKey(""); setReplacingApiKey(false); setDirty(false); }}>Discard Changes</button>
        <button className="button button-primary" disabled={!dirty || saving} onClick={() => void save()}>{saving ? "Saving…" : "Save Changes"}</button>
        <button className="button button-quiet danger-text" onClick={() => void reset()} disabled={resetting}>{resetting ? "Resetting…" : "Reset Local Config"}</button>
      </div>
    </div>
  );
}

export function FirstRunWizard({ onDone }: { onDone: () => void }) {
  const data = useHarnessData();
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (data.config) setForm(data.config);
  }, [data.config]);

  const chooseFolder = async () => {
    const path = await data.chooseWorkspace();
    if (path) {
      setForm((current) => ({ ...current, workspace: { rootPath: path } }));
      setError("");
      setMessage("");
    }
  };

  const saveWorkspace = async () => {
    if (data.configError) return setError("Reset the invalid local configuration before saving a Workspace.");
    if (!form.workspace.rootPath.trim()) return setError("Choose a Workspace folder.");
    setSaving(true);
    const result = await data.saveConfig(
      { ...form, tunnel: { ...form.tunnel, tunnelId: form.tunnel.tunnelId.trim() } },
      apiKey.trim() || undefined,
    );
    setSaving(false);
    setApiKey("");
    if (!result.ok) return setError(result.error?.message ?? "Could not save the Workspace.");
    setMessage("Configuration saved. Harness readiness has been refreshed.");
  };

  const resetConfiguration = async () => {
    setResetting(true);
    const result = await data.resetConfig();
    setResetting(false);
    if (!result.ok) return setError(result.error?.message ?? "Could not reset local configuration.");
    setForm(emptyConfig);
    setError("");
    setMessage("Invalid configuration was removed. Choose a Workspace to continue.");
  };

  const startHarness = async () => {
    const result = await data.startHarness();
    if (!result.ok) return setError(result.error?.message ?? "Harness could not start.");
    onDone();
  };

  const connectionConfigured = Boolean(
    form.tunnel.tunnelId.trim() && (data.apiKeyConfigured || apiKey.trim()),
  );

  return (
    <div className="onboarding-shell">
      <div className="wizard-content">
        <header className="wizard-header">
          <div className="brand-mark">H</div>
          <div><div className="eyebrow">First run</div><h1>ChatGPT Harness</h1><h2>Local Runtime Setup</h2></div>
        </header>
        <Card className="wizard-card">
          {data.configError ? (
            <div className="notice notice-danger"><strong>Configuration needs attention</strong><span>{data.configError.message}</span><button className="button button-quiet" onClick={() => void resetConfiguration()} disabled={resetting}>{resetting ? "Resetting…" : "Reset configuration"}</button></div>
          ) : (
            <>
              <section aria-labelledby="workspace-step-title">
                <SectionHeading title="Workspace" description="Choose the only folder Harness may access." />
                <label className="form-field"><span>Workspace root</span><div className="input-action-row"><input className="path-input" value={form.workspace.rootPath} readOnly placeholder="Choose a folder" title={form.workspace.rootPath || undefined} /><button className="button button-secondary" onClick={() => void chooseFolder()}>Choose Folder</button></div></label>
              </section>
              <section aria-labelledby="runtime-status-title">
                <SectionHeading title="Runtime" description="Bundled Runtime readiness is checked by the local control plane." />
                <p className="settings-help">{data.eligibility?.message ?? "Checking Harness readiness…"}</p>
              </section>
              <section aria-labelledby="external-connection-title">
                <SectionHeading title="Tunnel connection" description={connectionConfigured ? "Configured." : "Tunnel ID and API Key are required."} />
                <label className="form-field"><span>Tunnel ID (required)</span><input value={form.tunnel.tunnelId} onChange={(event) => setForm((current) => ({ ...current, tunnel: { ...current.tunnel, tunnelId: event.target.value } }))} placeholder="Enter tunnel ID" /></label>
                <label className="form-field"><span>API Key (required)</span><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder="Enter API Key" autoComplete="off" /></label>
                <p className="settings-help">The installed Tunnel client is bundled and resolved automatically.</p>
              </section>
            </>
          )}
        </Card>
        {error && <div className="notice notice-danger">{error}</div>}
        {message && <div className="notice notice-success">{message}</div>}
        {!data.configError && <div className="wizard-actions"><button className="button button-secondary" onClick={() => void saveWorkspace()} disabled={saving || !form.workspace.rootPath.trim()}>{saving ? "Saving…" : "Save Configuration"}</button><button className="button button-primary" onClick={() => void startHarness()} disabled={saving || !data.eligibility?.canStart}>Start Harness</button></div>}
      </div>
    </div>
  );
}
function SettingsSection({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <Card className="settings-section"><SectionHeading eyebrow={eyebrow} title={title} />{children}</Card>;
}


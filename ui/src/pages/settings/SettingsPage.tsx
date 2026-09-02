import { useEffect, useState, type ReactNode } from "react";
import type { AppConfig, DiagnosticRun, SecurityMode } from "@protocol/v1";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";

const emptyConfig: AppConfig = {
  schemaVersion: 1,
  configRevision: "0",
  workspace: { rootPath: "" },
  tunnel: { tunnelId: "", autoDetect: true },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
};

type WizardStep = 0 | 1 | 2 | 3;
const wizardSteps = ["Workspace", "Connection", "Security", "Diagnostics"] as const;

function checkLabel(id: string) {
  return ({
    workspace: "Workspace",
    tunnel_id: "Tunnel ID",
    tunnel_client: "Tunnel client",
    api_key: "API Key",
    security: "Security configuration",
  } as Record<string, string>)[id] ?? id;
}

function statusSymbol(status: string) {
  return status === "pass" ? "✓" : status === "warning" ? "!" : "×";
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
    else setError("No tunnel-client was found. Choose it manually.");
  };

  const setMode = (mode: SecurityMode) => {
    update({ security: { mode, developerTestMode: mode === "developer_test" } });
  };

  const save = async () => {
    setError("");
    if (!form.workspace.rootPath.trim()) return setError("Choose a workspace folder.");
    if (!form.tunnel.tunnelId.trim()) return setError("Tunnel ID is required.");
    if (!form.tunnel.tunnelClientPath?.trim()) return setError("Choose or detect a tunnel-client file.");
    if (replacingApiKey && !apiKey.trim()) return setError("Enter an API Key or cancel replacement.");

    setSaving(true);
    const result = await data.saveConfig(
      { ...form, tunnel: { ...form.tunnel, tunnelId: form.tunnel.tunnelId.trim() } },
      apiKey.trim() || undefined,
    );
    setSaving(false);
    if (!result.ok) return setError(result.error?.message ?? "Could not save configuration.");

    setApiKey("");
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
                value={form.workspace.rootPath}
                onChange={(event) => update({ workspace: { rootPath: event.target.value } })}
                placeholder="Choose a folder"
                title={form.workspace.rootPath || undefined}
              />
            </label>
            <button className="button button-secondary align-end" onClick={() => void chooseFolder()}>Choose Folder</button>
          </div>
          <p className="settings-help">Only this workspace will be used by the Harness.</p>
        </SettingsSection>

        <SettingsSection eyebrow="Connection" title="Tunnel client">
          <label className="form-field">
            <span>Tunnel ID</span>
            <input value={form.tunnel.tunnelId} onChange={(event) => update({ tunnel: { ...form.tunnel, tunnelId: event.target.value } })} placeholder="Enter tunnel ID" />
          </label>
          <div className="form-row">
            <label className="form-field grow">
              <span>Tunnel client</span>
              <input className="path-input" value={form.tunnel.tunnelClientPath ?? ""} readOnly placeholder="Not selected" title={form.tunnel.tunnelClientPath || undefined} />
            </label>
            <button className="button button-secondary align-end" onClick={() => void detect()}>Auto Detect</button>
            <button className="button button-secondary align-end" onClick={() => void chooseClient()}>Choose File</button>
          </div>
          <label className="form-field">
            <span>API Key</span>
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
  const [step, setStep] = useState<WizardStep>(0);
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [replacingApiKey, setReplacingApiKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [diagnostics, setDiagnostics] = useState<DiagnosticRun | null>(null);

  useEffect(() => {
    if (data.config) setForm(data.config);
  }, [data.config]);

  const update = (patch: Partial<AppConfig>) => {
    setForm((current) => ({ ...current, ...patch }));
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
    else setError("No tunnel-client was found. Choose it manually.");
  };

  const setMode = (mode: SecurityMode) => update({ security: { mode, developerTestMode: mode === "developer_test" } });

  const persist = async () => {
    setSaving(true);
    const result = await data.saveConfig(
      { ...form, tunnel: { ...form.tunnel, tunnelId: form.tunnel.tunnelId.trim() } },
      apiKey.trim() || undefined,
    );
    setSaving(false);
    if (!result.ok) {
      setError(result.error?.message ?? "Could not save configuration.");
      return false;
    }
    await data.refreshConfig();
    return true;
  };

  const continueFromWorkspace = () => {
    if (!form.workspace.rootPath.trim()) return setError("Choose a workspace folder to continue.");
    setError("");
    setStep(1);
  };

  const continueFromConnection = () => {
    if (!form.tunnel.tunnelId.trim()) return setError("Tunnel ID is required.");
    if (!form.tunnel.tunnelClientPath?.trim()) return setError("Choose or detect a tunnel-client file.");
    if (!apiKey.trim() && (!data.apiKeyConfigured || replacingApiKey)) return setError("Enter an API Key to continue.");
    setError("");
    setStep(2);
  };

  const continueFromSecurity = async () => {
    setError("");
    if (!(await persist())) return;
    try {
      setDiagnostics(await data.runDiagnostics());
      setStep(3);
    } catch (diagnosticError) {
      setError(diagnosticError instanceof Error ? diagnosticError.message : "Diagnostics could not run.");
    }
  };

  const fixIssue = (id: string) => {
    if (id === "workspace") setStep(0);
    else if (id === "tunnel_id" || id === "tunnel_client" || id === "api_key") setStep(1);
    else if (id === "security") setStep(2);
  };

  const finish = () => {
    if (!diagnostics) return setError("Diagnostics have not completed yet.");
    if (diagnostics.overallStatus === "fail") return setError("Diagnostics found a blocking issue. Fix it before finishing setup.");
    onDone();
  };

  const fallbackChecks = [
    { id: "workspace", status: "pass", summary: "Workspace configuration is ready." },
    { id: "tunnel_id", status: "pass", summary: "Tunnel ID is configured." },
    { id: "tunnel_client", status: form.tunnel.tunnelClientPath ? "pass" : "fail", summary: form.tunnel.tunnelClientPath ? "Tunnel client is selected." : "Choose or detect a tunnel client." },
    { id: "api_key", status: data.apiKeyConfigured || Boolean(apiKey.trim()) ? "pass" : "fail", summary: data.apiKeyConfigured || Boolean(apiKey.trim()) ? "API Key is configured securely." : "API Key is required." },
    { id: "security", status: "pass", summary: "Security configuration is selected." },
  ];
  const checks = diagnostics?.checks ?? fallbackChecks;

  return (
    <div className="onboarding-shell">
      <div className="wizard-content">
        <header className="wizard-header">
          <div className="brand-mark">H</div>
          <div>
            <div className="eyebrow">First run setup</div>
            <h1>ChatGPT Harness</h1>
            <h2>Connect your local workspace</h2>
          </div>
        </header>

        <div className="wizard-stepper" aria-label="Setup progress">
          {wizardSteps.map((label, index) => (
            <div className={`wizard-step ${index === step ? "active" : index < step ? "complete" : ""}`} key={label}>
              <span className="wizard-step-marker">{index < step ? "✓" : index + 1}</span>
              <span>{label}</span>
            </div>
          ))}
        </div>

        <Card className="wizard-card">
          {step === 0 && (
            <section aria-labelledby="workspace-step-title">
              <SectionHeading title="Workspace" description="Choose the project ChatGPT Harness may access." />
              <label className="form-field">
                <span>Workspace root</span>
                <div className="input-action-row">
                  <input className="path-input" value={form.workspace.rootPath} onChange={(event) => update({ workspace: { rootPath: event.target.value } })} placeholder="Choose a folder" title={form.workspace.rootPath || undefined} />
                  <button className="button button-secondary" onClick={() => void chooseFolder()}>Choose Folder</button>
                </div>
              </label>
              <p className="settings-help">Only this workspace will be used by the Harness.</p>
            </section>
          )}

          {step === 1 && (
            <section aria-labelledby="connection-step-title">
              <SectionHeading title="Connection" description="Connect the local tunnel client to your workspace." />
              <label className="form-field"><span>Tunnel ID</span><input value={form.tunnel.tunnelId} onChange={(event) => update({ tunnel: { ...form.tunnel, tunnelId: event.target.value } })} placeholder="Enter tunnel ID" /></label>
              <label className="form-field"><span>Tunnel client</span><div className="input-action-row"><input className="path-input" value={form.tunnel.tunnelClientPath ?? ""} readOnly placeholder="Auto-detect or choose a file" title={form.tunnel.tunnelClientPath || undefined} /><button className="button button-secondary" onClick={() => void detect()}>Auto Detect</button><button className="button button-secondary" onClick={() => void chooseClient()}>Choose File</button></div></label>
              <label className="form-field">
                <span>API Key</span>
                {data.apiKeyConfigured && !replacingApiKey ? (
                  <div className="secret-state"><StatusBadge tone="success">Configured</StatusBadge><button className="button button-quiet" onClick={() => { setReplacingApiKey(true); setApiKey(""); }}>Replace</button><button className="button button-quiet danger-text" onClick={() => { setReplacingApiKey(false); setApiKey(""); void data.deleteApiKey(); }}>Remove</button></div>
                ) : (
                  <input type="password" value={apiKey} onChange={(event) => { setApiKey(event.target.value); setError(""); }} placeholder="Enter API Key" autoComplete="off" />
                )}
              </label>
            </section>
          )}

          {step === 2 && (
            <section aria-labelledby="security-step-title">
              <SectionHeading title="Security" description="Choose the capability profile for this local Harness." />
              <div className="mode-options wizard-mode-options">
                <button className={`mode-option ${form.security.mode === "normal" ? "selected" : ""}`} onClick={() => setMode("normal")}><strong>Normal Mode</strong><span className="mode-recommended">Recommended</span><span>Read-only default capabilities.</span></button>
                <button className={`mode-option developer ${form.security.mode === "developer_test" ? "selected" : ""}`} onClick={() => setMode("developer_test")}><strong>Developer/Test Mode</strong><span className="mode-warning">Higher risk</span><span>Higher-risk developer capabilities may become available in later milestones.</span></button>
              </div>
            </section>
          )}

          {step === 3 && (
            <section aria-labelledby="diagnostics-step-title">
              <SectionHeading title="Diagnostics" description="Review the checks that can run before Runtime integration." />
              <div className="diagnostic-list wizard-diagnostics">
                {checks.map((check) => (
                  <div className="wizard-check" key={check.id}>
                    <span className={`wizard-check-icon ${check.status}`}>{statusSymbol(check.status)}</span>
                    <div>
                      <strong>{checkLabel(check.id)}</strong>
                      <p>{check.summary}</p>
                      {check.status === "warning" && <small>Available after Runtime integration</small>}
                      {check.status === "fail" && <button className="button button-quiet" onClick={() => fixIssue(check.id)}>Fix issue</button>}
                    </div>
                  </div>
                ))}
                <div className="wizard-check pending"><span className="wizard-check-icon">○</span><div><strong>Runtime</strong><p>Available after Runtime integration</p></div></div>
                <div className="wizard-check pending"><span className="wizard-check-icon">○</span><div><strong>MCP</strong><p>Available after Runtime integration</p></div></div>
              </div>
            </section>
          )}
        </Card>

        {error && <div className="notice notice-danger">{error}</div>}
        <div className="wizard-actions">
          <button className="button button-quiet" onClick={() => { if (step > 0) setStep((step - 1) as WizardStep); }} disabled={step === 0 || saving}>Back</button>
          {step < 2 && <button className="button button-primary" onClick={step === 0 ? continueFromWorkspace : continueFromConnection} disabled={saving}>Continue <span aria-hidden="true">→</span></button>}
          {step === 2 && <button className="button button-primary" onClick={() => void continueFromSecurity()} disabled={saving}>{saving ? "Saving…" : "Continue"} <span aria-hidden="true">→</span></button>}
          {step === 3 && <button className="button button-primary" onClick={finish} disabled={saving}>{saving ? "Running…" : "Finish Setup"}</button>}
        </div>
      </div>
    </div>
  );
}

function SettingsSection({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <Card className="settings-section"><SectionHeading eyebrow={eyebrow} title={title} />{children}</Card>;
}


import { useEffect, useState } from "react";
import type { AppConfig, DiagnosticRun, SecurityMode } from "@protocol/v1";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";

const emptyConfig: AppConfig = {
  schemaVersion: 1,
  configRevision: 0n,
  workspace: { rootPath: "" },
  tunnel: { tunnelId: "", autoDetect: true },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
};
const diagnosticTone = (status: string) =>
  status === "pass" ? "success" : status === "warning" ? "warning" : "danger";

export function SettingsPage({ setup = false, onDone }: { setup?: boolean; onDone?: () => void }) {
  const data = useHarnessData();
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [diagnostics, setDiagnostics] = useState<DiagnosticRun | null>(null);
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
  const save = async () => {
    setError("");
    if (!form.workspace.rootPath.trim()) return setError("Choose a workspace folder.");
    if (!form.tunnel.tunnelId.trim()) return setError("Tunnel ID is required.");
    if (!form.tunnel.tunnelClientPath?.trim()) return setError("Choose or detect a tunnel-client file.");
    if (setup && !apiKey.trim() && !data.apiKeyConfigured) return setError("Enter an API Key to continue.");
    setSaving(true);
    const result = await data.saveConfig(
      { ...form, tunnel: { ...form.tunnel, tunnelId: form.tunnel.tunnelId.trim() } },
      apiKey || undefined,
    );
    setSaving(false);
    if (!result.ok) return setError(result.error?.message ?? "Could not save configuration.");
    setApiKey("");
    setDirty(false);
    setMessage("Changes saved.");
    await data.refreshConfig();
    if (setup) {
      try {
        const run = await data.runDiagnostics();
        setDiagnostics(run);
        if (run.overallStatus === "fail") {
          setError("Configuration was saved, but diagnostics found a blocking issue.");
          return;
        }
        onDone?.();
      } catch (diagnosticError) {
        setError(diagnosticError instanceof Error ? diagnosticError.message : "Diagnostics could not run.");
      }
    }
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
  const setMode = (mode: SecurityMode) =>
    update({ security: { mode, developerTestMode: mode === "developer_test" } });
  const reset = async () => {
    if (!window.confirm("Reset local configuration and remove the stored API Key?")) return;
    setResetting(true);
    const result = await data.resetConfig();
    setResetting(false);
    if (!result.ok) return setError(result.error?.message ?? "Could not reset configuration.");
    setForm(emptyConfig);
    setApiKey("");
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
    <div className="page-stack">
      {setup && (
        <Card className="setup-hero">
          <div className="eyebrow">First run setup</div>
          <h2>Connect your local workspace</h2>
          <p>Set up the local control plane. Nothing is sent anywhere by this form.</p>
          <div className="setup-steps">
            <span className="active">1 Workspace</span><span>2 Connection</span><span>3 Security</span><span>4 Diagnostics</span>
          </div>
        </Card>
      )}
      {data.configError && (
        <div className="notice notice-danger">
          <strong>Configuration needs attention</strong><span>{data.configError.message}</span>
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
      <Card>
        <SectionHeading eyebrow="Workspace" title="Active workspace" description="Folder access is limited to the selected root." />
        <div className="form-row"><label className="form-field grow"><span>Workspace root</span><input value={form.workspace.rootPath} onChange={(e) => update({ workspace: { rootPath: e.target.value } })} placeholder="Choose a folder" /></label><button className="button button-secondary align-end" onClick={() => void chooseFolder()}>Choose Folder</button></div>
      </Card>
      <Card>
        <SectionHeading eyebrow="Connection" title="Tunnel client" description="M2 locates the client but does not start it." />
        <div className="form-field"><span>Tunnel ID</span><input value={form.tunnel.tunnelId} onChange={(e) => update({ tunnel: { ...form.tunnel, tunnelId: e.target.value } })} placeholder="Enter tunnel ID" /></div>
        <div className="form-row"><label className="form-field grow"><span>Client path</span><input value={form.tunnel.tunnelClientPath ?? ""} readOnly placeholder="Not selected" /></label><button className="button button-secondary align-end" onClick={() => void detect()}>Auto Detect</button><button className="button button-secondary align-end" onClick={() => void chooseClient()}>Choose File</button></div>
        <div className="form-field"><span>API Key</span>{data.apiKeyConfigured ? <div className="secret-state"><StatusBadge tone="success">Configured</StatusBadge><button className="button button-quiet" onClick={() => setApiKey("")}>Replace</button><button className="button button-quiet danger-text" onClick={() => { void data.deleteApiKey(); update({ tunnel: { ...form.tunnel, apiKeyRef: undefined } }); }}>Remove</button></div> : <input type="password" value={apiKey} onChange={(e) => { setApiKey(e.target.value); setDirty(true); }} placeholder="Enter API Key" autoComplete="off" />}</div>
      </Card>
      <Card>
        <SectionHeading eyebrow="Security" title="Security mode" description="The two M0 fields are kept consistent when saved." />
        <div className="mode-options"><button className={`mode-option ${form.security.mode === "normal" ? "selected" : ""}`} onClick={() => setMode("normal")}><strong>Normal Mode</strong><span>Read-only default capability set.</span></button><button className={`mode-option developer ${form.security.mode === "developer_test" ? "selected" : ""}`} onClick={() => setMode("developer_test")}><strong>Developer/Test Mode</strong><span>May expose higher-risk capabilities in later milestones.</span></button></div>
      </Card>
      <Card>
        <SectionHeading eyebrow="Local data" title="Storage boundaries" />
        <div className="local-facts"><div><span>Config directory</span><code>{data.configMetadata?.configDirectory ?? "OS application data"}</code></div><div><span>Config file</span><code>{data.configMetadata?.configFile ?? "config.json"}</code></div><div><span>Secret provider</span><code>{data.config?.tunnel.apiKeyRef?.provider ?? "OS credential store"}</code></div></div>
      </Card>
      {diagnostics && <Card><SectionHeading eyebrow="Diagnostics" title="First-run checks" /><div className="diagnostic-list">{diagnostics.checks.map((check) => <div className="diagnostic-row" key={check.id}><StatusBadge tone={diagnosticTone(check.status) as "success" | "warning" | "danger"}>{check.status}</StatusBadge><div><strong>{check.id}</strong><p>{check.summary}</p>{check.remediation && <small>{check.remediation}</small>}</div></div>)}</div></Card>}
      {error && <div className="notice notice-danger">{error}</div>}{message && <div className="notice notice-success">{message}</div>}
      <div className="settings-actions"><span className={dirty ? "warning-text" : "muted"}>{dirty ? "Unsaved changes" : "All changes saved"}</span><button className="button button-secondary" disabled={!dirty || saving} onClick={() => { setForm(data.config ?? emptyConfig); setDirty(false); }}>Discard Changes</button><button className="button button-primary" disabled={!dirty || saving} onClick={() => void save()}>{saving ? "Saving…" : "Save Changes"}</button>{!setup && <button className="button button-quiet danger-text" onClick={() => void reset()} disabled={resetting}>{resetting ? "Resetting…" : "Reset Local Config"}</button>}</div>
    </div>
  );
}

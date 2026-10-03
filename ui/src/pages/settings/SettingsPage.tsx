import { useEffect, useState, type ReactNode } from "react";
import type { AppConfig, SecurityMode, UiLanguage } from "@protocol/v1";
import { useTranslation } from "react-i18next";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData, type SaveResult } from "@/app/providers/HarnessDataProvider";
import { i18n } from "@/i18n";
import { resolveLanguage, systemLocales } from "@/i18n/resolve-language";
import { presentError, startEligibilityKey, type ErrorPresentation, type TranslationKey } from "@/i18n/presentation";

const emptyConfig: AppConfig = {
  bash: { autoDetect: true },
  schemaVersion: 1,
  configRevision: "0",
  workspace: { rootPath: "" },
  tunnel: { tunnelId: "", autoDetect: true },
  security: { mode: "normal", developerTestMode: false },
  ui: {},
  tools: { readFile: { enabled: true }, glob: { enabled: true }, write: { enabled: true }, edit: { enabled: true }, bash: { enabled: false } },
  policies: { toolDefaults: { readFile: "allow", glob: "allow", write: "allow", edit: "allow", bash: "ask" }, userRules: [] },
};

type ProxyValidationKey =
  | "validation.settings.proxyUrl"
  | "validation.settings.proxyProtocol"
  | "validation.settings.proxyHost"
  | "validation.settings.proxyCredentials"
  | "validation.settings.proxyPath";

export function validateControlPlaneProxy(value: string): ProxyValidationKey | "" {
  const trimmed = value.trim();
  if (!trimmed) return "";
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return "validation.settings.proxyUrl";
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return "validation.settings.proxyProtocol";
  if (!parsed.hostname) return "validation.settings.proxyHost";
  if (parsed.username || parsed.password) return "validation.settings.proxyCredentials";
  if (parsed.pathname !== "/" || parsed.search || parsed.hash) return "validation.settings.proxyPath";
  return "";
}

type Feedback =
  | { kind: "success"; key: TranslationKey }
  | ({ kind: "error" } & ErrorPresentation);

function errorFeedback(error: unknown): Feedback {
  return { kind: "error", ...presentError(error) };
}

function FeedbackNotice({ feedback }: { feedback: Feedback | null }) {
  const { t } = useTranslation();
  if (!feedback) return null;
  if (feedback.kind === "success") {
    return <div className="notice notice-success" role="status" aria-live="polite">{t(feedback.key)}</div>;
  }
  return (
    <div className="notice notice-danger" role="alert">
      <span>{t(feedback.summaryKey)}</span>
      {feedback.code && <code className="mono">{feedback.code}</code>}
      {feedback.detail && <span>{feedback.detail}</span>}
    </div>
  );
}

function saveError(result: SaveResult): Feedback {
  return result.error ? errorFeedback(result.error) : errorFeedback(undefined);
}

function currentLocale(): UiLanguage {
  return i18n.resolvedLanguage === "zh-CN" ? "zh-CN" : "en";
}

function settingUi(config: AppConfig, persisted: AppConfig | null): AppConfig["ui"] {
  // A language-only mutation may have happened while the Settings draft was
  // dirty. Prefer the latest persisted UI object so a later full save cannot
  // write the stale language back over it.
  return persisted ? { ...persisted.ui } : { ...config.ui };
}

export function SettingsPage() {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [replacingApiKey, setReplacingApiKey] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [proxyError, setProxyError] = useState<ProxyValidationKey | "">("");
  const [resetting, setResetting] = useState(false);
  const [languageSaving, setLanguageSaving] = useState(false);
  const [languageSelection, setLanguageSelection] = useState<UiLanguage | "">(() => data.config?.ui.language ?? "");

  useEffect(() => {
    if (!dirty) setForm(data.config ?? emptyConfig);
  }, [data.config, dirty]);

  useEffect(() => {
    if (!languageSaving) setLanguageSelection(data.config?.ui.language ?? "");
  }, [data.config, languageSaving]);

  const update = (patch: Partial<AppConfig>) => {
    setForm((current) => ({ ...current, ...patch }));
    setDirty(true);
    setFeedback(null);
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
    else setFeedback({ kind: "success", key: "settings.tunnel.bundledNotice" });
  };

  const setMode = (mode: SecurityMode) => {
    update({ security: { mode, developerTestMode: mode === "developer_test" } });
  };

  const changeLanguage = async (value: UiLanguage | "") => {
    if (languageSaving || saving || resetting) return;
    const previousPreference = data.config?.ui.language ?? null;
    const previousLocale = currentLocale();
    const nextPreference = value || null;
    const nextLocale = nextPreference ?? resolveLanguage(undefined, systemLocales());
    setLanguageSelection(value);
    setLanguageSaving(true);
    setFeedback(null);
    await i18n.changeLanguage(nextLocale);

    let result: SaveResult;
    try {
      result = await data.setUiLanguage(nextPreference);
    } catch (error) {
      const failed = errorFeedback(error);
      await i18n.changeLanguage(previousLocale);
      setLanguageSelection(previousPreference ?? "");
      setFeedback(failed);
      setLanguageSaving(false);
      return;
    }
    if (!result.ok) {
      await i18n.changeLanguage(previousLocale);
      setLanguageSelection(previousPreference ?? "");
      setFeedback(saveError(result));
    } else {
      setFeedback({ kind: "success", key: "settings.languageSaved" });
    }
    setLanguageSaving(false);
  };

  const save = async () => {
    if (languageSaving || saving || resetting) return;
    setFeedback(null);
    if (!form.workspace.rootPath.trim()) {
      setFeedback({ kind: "error", summaryKey: "validation.settings.workspace" });
      return;
    }
    const nextProxy = form.tunnel.controlPlaneHttpProxy?.trim() ?? "";
    const nextProxyError = validateControlPlaneProxy(nextProxy);
    setProxyError(nextProxyError);
    if (nextProxyError) return;
    setSaving(true);
    const result = await data.saveConfig(
      {
        ...form,
        ui: settingUi(form, data.config),
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
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }

    setReplacingApiKey(false);
    setDirty(false);
    setFeedback({ kind: "success", key: "settings.actions.savedNotice" });
    await data.refreshConfig();
  };

  const reset = async () => {
    if (languageSaving || saving || resetting) return;
    if (!window.confirm(t("settings.actions.resetConfirm"))) return;
    setResetting(true);
    const result = await data.resetConfig();
    setResetting(false);
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }
    setForm(emptyConfig);
    setApiKey("");
    setReplacingApiKey(false);
    setDirty(false);
    setFeedback({ kind: "success", key: "settings.actions.resetSuccess" });
  };

  const restartHarness = async () => {
    setFeedback(null);
    const result = await data.restartHarness();
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }
    setFeedback({ kind: "success", key: "settings.restart.success" });
  };

  return (
    <div className="settings-page page-stack">
      <Card className="settings-banner">
        <div className="settings-lock" aria-hidden="true">⌁</div>
        <div>
          <StatusBadge tone="success">{t("settings.banner.badge")}</StatusBadge>
          <h2>{t("settings.banner.title")}</h2>
          <p>{t("settings.banner.description")}</p>
        </div>
      </Card>

      <SettingsSection eyebrow={t("settings.language")} title={t("settings.language")}>
        <label className="form-field">
          <span>{t("settings.language")}</span>
          <select
            value={languageSelection}
            onChange={(event) => void changeLanguage(event.target.value as UiLanguage | "")}
            disabled={languageSaving || saving || resetting}
            aria-busy={languageSaving}
          >
            <option value="">{t("settings.followSystem")}</option>
            <option value="en">{t("settings.languages.en")}</option>
            <option value="zh-CN">{t("settings.languages.zhCN")}</option>
          </select>
        </label>
        <p className="settings-help">{languageSaving ? t("settings.languageSaving") : t("settings.languageHelp")}</p>
      </SettingsSection>

      {data.configError && (
        <div className="notice notice-danger" role="alert">
          <strong>{t("settings.configError.title")}</strong>
          <span>{t(presentError(data.configError).summaryKey)}</span>
          {presentError(data.configError).code && <code className="mono">{presentError(data.configError).code}</code>}
          <button className="button button-quiet" onClick={() => void reset()} disabled={resetting || saving || languageSaving}>{t("settings.configError.reset")}</button>
        </div>
      )}
      {data.restartRequired && (
        <div className="notice notice-warning" role="status">
          <strong>{t("settings.restart.title")}</strong>
          <span>{t("settings.restart.description")}</span>
          <button className="button button-warning" onClick={() => void restartHarness()}>{t("settings.restart.action")}</button>
        </div>
      )}

      <div className="settings-layout">
        <SettingsSection eyebrow={t("settings.workspace.eyebrow")} title={t("settings.workspace.title")}>
          <div className="form-row">
            <label className="form-field grow">
              <span>{t("settings.workspace.root")}</span>
              <input
                className="path-input"
                value={form.workspace.rootPath} readOnly
                placeholder={t("settings.workspace.placeholder")}
                title={form.workspace.rootPath || undefined}
              />
            </label>
            <button className="button button-secondary align-end" onClick={() => void chooseFolder()}>{t("settings.workspace.choose")}</button>
          </div>
          <p className="settings-help">{t("settings.workspace.help")}</p>
        </SettingsSection>

        <SettingsSection eyebrow={t("settings.tunnel.eyebrow")} title={t("settings.tunnel.title")}>
          <label className="form-field">
            <span>{t("settings.tunnel.id")}</span>
            <input aria-label={t("settings.tunnel.id")} value={form.tunnel.tunnelId} onChange={(event) => update({ tunnel: { ...form.tunnel, tunnelId: event.target.value } })} placeholder={t("settings.tunnel.idPlaceholder")} />
          </label>
          <label className="form-field">
            <span>{t("settings.tunnel.proxy")}</span>
            <input
              aria-label={t("settings.tunnel.proxy")}
              value={form.tunnel.controlPlaneHttpProxy ?? ""}
              onChange={(event) => {
                setProxyError("");
                update({ tunnel: { ...form.tunnel, controlPlaneHttpProxy: event.target.value || undefined } });
              }}
              placeholder={t("settings.tunnel.proxyPlaceholder")}
              aria-invalid={Boolean(proxyError)}
              aria-describedby={proxyError ? "control-plane-proxy-error" : undefined}
            />
            {proxyError && <span id="control-plane-proxy-error" className="settings-help danger-text" role="alert">{t(proxyError)}</span>}
          </label>
          <p className="settings-help">{t("settings.tunnel.proxyHelp")}</p>
          <div className="form-row">
            <label className="form-field grow">
              <span>{t("settings.tunnel.client")}</span>
              <input className="path-input" value={form.tunnel.tunnelClientPath ?? ""} readOnly placeholder={t("settings.tunnel.clientPlaceholder")} title={form.tunnel.tunnelClientPath || undefined} />
            </label>
            <button className="button button-secondary align-end" onClick={() => void detect()}>{t("settings.tunnel.useBundled")}</button>
            <button className="button button-secondary align-end" onClick={() => void chooseClient()}>{t("settings.tunnel.chooseOverride")}</button>
          </div>
          <label className="form-field">
            <span>{t("settings.tunnel.apiKey")}</span>
            {data.apiKeyConfigured && !replacingApiKey ? (
              <div className="secret-state">
                <StatusBadge tone="success">{t("settings.tunnel.configured")}</StatusBadge>
                <button className="button button-quiet" onClick={() => { setReplacingApiKey(true); setApiKey(""); }}>{t("settings.tunnel.replace")}</button>
                <button className="button button-quiet danger-text" onClick={() => { setReplacingApiKey(false); setApiKey(""); void data.deleteApiKey(); }}>{t("settings.tunnel.remove")}</button>
              </div>
            ) : (
              <input type="password" aria-label={t("settings.tunnel.apiKey")} value={apiKey} onChange={(event) => { setApiKey(event.target.value); setDirty(true); setFeedback(null); }} placeholder={t("settings.tunnel.apiKeyPlaceholder")} autoComplete="off" />
            )}
          </label>
        </SettingsSection>

        <SettingsSection eyebrow={t("settings.security.eyebrow")} title={t("settings.security.title")}>
          <div className="mode-options">
            <button className={`mode-option ${form.security.mode === "normal" ? "selected" : ""}`} onClick={() => setMode("normal")} aria-pressed={form.security.mode === "normal"}>
              <strong>{t("settings.security.normal")}</strong>
              <span>{t("settings.security.normalDescription")}</span>
            </button>
            <button className={`mode-option developer ${form.security.mode === "developer_test" ? "selected" : ""}`} onClick={() => setMode("developer_test")} aria-pressed={form.security.mode === "developer_test"}>
              <strong>{t("settings.security.developerTest")}</strong>
              <span>{t("settings.security.developerDescription")}</span>
            </button>
          </div>
        </SettingsSection>

        <SettingsSection eyebrow={t("settings.localData.eyebrow")} title={t("settings.localData.title")}>
          <div className="local-facts">
            <div><span>{t("settings.localData.configDirectory")}</span><code className="path-value" title={data.configMetadata?.configDirectory}>{data.configMetadata?.configDirectory ?? t("settings.localData.osApplicationData")}</code></div>
            <div><span>{t("settings.localData.configFile")}</span><code className="path-value" title={data.configMetadata?.configFile}>{data.configMetadata?.configFile ?? t("settings.localData.configJson")}</code></div>
            <div><span>{t("settings.localData.secretProvider")}</span><code>{data.config?.tunnel.apiKeyRef?.provider ?? t("settings.localData.credentialStore")}</code></div>
          </div>
        </SettingsSection>
      </div>

      <FeedbackNotice feedback={feedback} />
      <div className="settings-actions">
        <span className={dirty ? "warning-text" : "muted"}>{dirty ? t("settings.actions.unsaved") : t("settings.actions.saved")}</span>
        <button className="button button-secondary" disabled={!dirty || saving || resetting || languageSaving} onClick={() => { setForm(data.config ?? emptyConfig); setApiKey(""); setReplacingApiKey(false); setDirty(false); }}>{t("settings.actions.discard")}</button>
        <button className="button button-primary" disabled={!dirty || saving || resetting || languageSaving} onClick={() => void save()}>{saving ? t("settings.actions.saving") : t("settings.actions.save")}</button>
        <button className="button button-quiet danger-text" onClick={() => void reset()} disabled={resetting || saving || languageSaving}>{resetting ? t("settings.actions.resetting") : t("settings.actions.reset")}</button>
      </div>
    </div>
  );
}

export function FirstRunWizard({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [form, setForm] = useState<AppConfig>(data.config ?? emptyConfig);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (data.config) setForm(data.config);
  }, [data.config]);

  const chooseFolder = async () => {
    const path = await data.chooseWorkspace();
    if (path) {
      setForm((current) => ({ ...current, workspace: { rootPath: path } }));
      setFeedback(null);
    }
  };

  const saveWorkspace = async () => {
    if (data.configError) {
      setFeedback({ kind: "error", summaryKey: "firstRun.invalidConfigReset" });
      return;
    }
    if (!form.workspace.rootPath.trim()) {
      setFeedback({ kind: "error", summaryKey: "firstRun.workspaceRequired" });
      return;
    }
    setSaving(true);
    const result = await data.saveConfig(
      {
        ...form,
        ui: settingUi(form, data.config),
        tunnel: { ...form.tunnel, tunnelId: form.tunnel.tunnelId.trim() },
      },
      apiKey.trim() || undefined,
    );
    setSaving(false);
    setApiKey("");
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }
    setFeedback({ kind: "success", key: "firstRun.saveSuccess" });
  };

  const resetConfiguration = async () => {
    setResetting(true);
    const result = await data.resetConfig();
    setResetting(false);
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }
    setForm(emptyConfig);
    setFeedback({ kind: "success", key: "firstRun.resetSuccess" });
  };

  const startHarness = async () => {
    const result = await data.startHarness();
    if (!result.ok) {
      setFeedback(saveError(result));
      return;
    }
    onDone();
  };

  const connectionConfigured = Boolean(
    form.tunnel.tunnelId.trim() && (data.apiKeyConfigured || apiKey.trim()),
  );

  return (
    <div className="onboarding-shell">
      <div className="wizard-content">
        <header className="wizard-header">
          <div className="brand-mark" aria-hidden="true">H</div>
          <div><div className="eyebrow">{t("firstRun.eyebrow")}</div><h1>ChatGPT Harness</h1><h2>{t("firstRun.title")}</h2></div>
        </header>
        <Card className="wizard-card">
          {data.configError ? (
            <div className="notice notice-danger" role="alert"><strong>{t("firstRun.configErrorTitle")}</strong><span>{t(presentError(data.configError).summaryKey)}</span><button className="button button-quiet" onClick={() => void resetConfiguration()} disabled={resetting}>{resetting ? t("firstRun.resetting") : t("firstRun.reset")}</button></div>
          ) : (
            <>
              <section aria-labelledby="workspace-step-title">
                <SectionHeading title={t("firstRun.workspaceTitle")} description={t("firstRun.workspaceDescription")} />
                <label className="form-field"><span>{t("firstRun.workspaceRoot")}</span><div className="input-action-row"><input className="path-input" value={form.workspace.rootPath} readOnly placeholder={t("settings.workspace.placeholder")} title={form.workspace.rootPath || undefined} /><button className="button button-secondary" onClick={() => void chooseFolder()}>{t("firstRun.chooseFolder")}</button></div></label>
              </section>
              <section aria-labelledby="runtime-status-title">
                <SectionHeading title={t("firstRun.runtimeTitle")} description={t("firstRun.runtimeDescription")} />
                <p className="settings-help">{data.eligibility ? t(startEligibilityKey(data.eligibility.status)) : t("firstRun.checking")}</p>
              </section>
              <section aria-labelledby="external-connection-title">
                <SectionHeading title={t("firstRun.tunnelTitle")} description={connectionConfigured ? t("firstRun.configured") : t("firstRun.requirements")} />
                <label className="form-field"><span>{t("firstRun.tunnelId")}</span><input aria-label={t("firstRun.tunnelId")} value={form.tunnel.tunnelId} onChange={(event) => setForm((current) => ({ ...current, tunnel: { ...current.tunnel, tunnelId: event.target.value } }))} placeholder={t("firstRun.tunnelIdPlaceholder")} /></label>
                <label className="form-field"><span>{t("firstRun.apiKey")}</span><input type="password" aria-label={t("firstRun.apiKey")} value={apiKey} onChange={(event) => setApiKey(event.target.value)} placeholder={t("firstRun.apiKeyPlaceholder")} autoComplete="off" /></label>
                <p className="settings-help">{t("firstRun.tunnelHelp")}</p>
              </section>
            </>
          )}
        </Card>
        <FeedbackNotice feedback={feedback} />
        {!data.configError && <div className="wizard-actions"><button className="button button-secondary" onClick={() => void saveWorkspace()} disabled={saving || !form.workspace.rootPath.trim()}>{saving ? t("firstRun.saving") : t("firstRun.save")}</button><button className="button button-primary" onClick={() => void startHarness()} disabled={saving || !data.eligibility?.canStart}>{t("firstRun.start")}</button></div>}
      </div>
    </div>
  );
}

function SettingsSection({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <Card className="settings-section"><SectionHeading eyebrow={eyebrow} title={title} />{children}</Card>;
}

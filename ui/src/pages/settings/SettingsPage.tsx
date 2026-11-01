import { ConfigRecovery } from "@/components/status/ConfigRecovery";
import { DiagnosticsPanel } from "@/components/status/DiagnosticsPanel";
import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import { useEffect, useState, type ReactNode } from "react";
import type { SecurityMode, TunnelConfig, UiLanguage } from "@protocol/v1";
import { useTranslation } from "react-i18next";
import { Button, Dialog, Disclosure, InlineFeedback, Row, Section, Segmented, Select, Sheet, StatusBadge, TextField } from "@/components/common/primitives";
import { useHarnessData, type SaveResult } from "@/app/providers/HarnessDataProvider";
import { i18n } from "@/i18n";
import { resolveLanguage, systemLocales } from "@/i18n/resolve-language";
import { presentError, securityModeKey, type ErrorPresentation, type TranslationKey } from "@/i18n/presentation";
import { useDraftGuard, useDraftNavigation } from "@/app/router/DraftNavigation";
import type { Navigate, NavigationOptions } from "@/app/router/routes";
import { assertUnchanged } from "@/app/providers/config-mutations";
import type { DesktopPreferencesPatch } from "@/services/harness/client";
import { validateControlPlaneProxy } from "./configuration";
import { normalizeThemePreference, type ThemePreference } from "@/theme/theme";
import { HistorySettingsSection } from "./HistorySettingsSection";

export { validateControlPlaneProxy } from "./configuration";

type ProxyValidationKey = ReturnType<typeof validateControlPlaneProxy>;

type TunnelDraft = {
  tunnelId: string;
  controlPlaneHttpProxy: string;
  tunnelClientPath: string;
  autoDetect: boolean;
};

function tunnelDraft(tunnel: TunnelConfig): TunnelDraft {
  return {
    tunnelId: tunnel.tunnelId,
    controlPlaneHttpProxy: tunnel.controlPlaneHttpProxy ?? "",
    tunnelClientPath: tunnel.tunnelClientPath ?? "",
    autoDetect: tunnel.autoDetect,
  };
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

export function SettingsPage({ initialSection, onNavigate }: { initialSection?: NavigationOptions["settingsSection"]; onNavigate?: Navigate } = {}) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const { requestLeave, finishDraft } = useDraftNavigation();
  const [languageSelection, setLanguageSelection] = useState<UiLanguage | "">(() => data.config?.ui.language ?? "");
  const [themeSelection, setThemeSelection] = useState<ThemePreference>(() => normalizeThemePreference(data.config?.ui.theme));
  const [languageSaving, setLanguageSaving] = useState(false);
  const [languageFeedback, setLanguageFeedback] = useState<Feedback | null>(null);
  const [themeSaving, setThemeSaving] = useState(false);
  const [themeFeedback, setThemeFeedback] = useState<Feedback | null>(null);
  const [notificationSaving, setNotificationSaving] = useState(false);
  const [notificationFeedback, setNotificationFeedback] = useState<Feedback | null>(null);
  const [workspaceSaving, setWorkspaceSaving] = useState(false);
  const [workspaceFeedback, setWorkspaceFeedback] = useState<Feedback | null>(null);
  const [connectionOpen, setConnectionOpen] = useState(false);
  const [connectionObserved, setConnectionObserved] = useState<TunnelConfig | null>(null);
  const [connectionSaving, setConnectionSaving] = useState(false);
  const [connectionDirty, setConnectionDirty] = useState(false);
  const [connectionFeedback, setConnectionFeedback] = useState<Feedback | null>(null);
  const [removeConfirmation, setRemoveConfirmation] = useState(false);
  const [removingApiKey, setRemovingApiKey] = useState(false);
  const [securitySaving, setSecuritySaving] = useState(false);
  const [securityFeedback, setSecurityFeedback] = useState<Feedback | null>(null);
  const [developerConfirmation, setDeveloperConfirmation] = useState(false);
  const [resetConfirmation, setResetConfirmation] = useState(false);
  const [resetFeedback, setResetFeedback] = useState<Feedback | null>(null);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (!languageSaving) setLanguageSelection(data.config?.ui.language ?? "");
  }, [data.config?.ui.language, languageSaving]);

  useEffect(() => {
    if (!themeSaving) setThemeSelection(normalizeThemePreference(data.config?.ui.theme));
  }, [data.config?.ui.theme, themeSaving]);

  useEffect(() => {
    if (!initialSection) return;
    const node = document.getElementById(`settings-section-${initialSection}`);
    node?.scrollIntoView({ block: "start", behavior: "auto" });
    node?.focus();
  }, [initialSection]);

  const currentConfig = data.config;
  const savedMode = currentConfig?.security.mode;
  const components = data.components;
  const runtime = data.runtime;
  const running = components.runtime === "ready" || components.runtime === "degraded";
  const observedRuntime = runtime.status === "success" || runtime.status === "empty" ? runtime.data : null;
  const controlPlaneConnected = data.harness?.tunnelReadiness?.controlPlaneConnected;
  const tunnelObservedReady = components.tunnel === "ready";
  const connectionStatus = controlPlaneConnected === true && tunnelObservedReady
    ? t("status.ready")
    : controlPlaneConnected === false
      ? t("status.error")
      : t("m8.unknown");
  const apiKeyStatus = data.apiKeyObserved
    ? data.apiKeyConfigured ? "configured" : "missing"
    : "unknown";

  const changeLanguage = async (value: UiLanguage | "") => {
    if (languageSaving || resetting) return;
    const previousPreference = data.config?.ui.language ?? null;
    const previousLocale = currentLocale();
    const nextPreference = value || null;
    const nextLocale = nextPreference ?? resolveLanguage(undefined, systemLocales());
    setLanguageSelection(value);
    setLanguageSaving(true);
    setLanguageFeedback(null);
    await i18n.changeLanguage(nextLocale);
    try {
      const result = await data.setUiLanguage(nextPreference);
      if (!result.ok) {
        await i18n.changeLanguage(previousLocale);
        setLanguageSelection(previousPreference ?? "");
        setLanguageFeedback(saveError(result));
      } else {
        setLanguageFeedback({ kind: "success", key: "settings.languageSaved" });
      }
    } catch (error) {
      await i18n.changeLanguage(previousLocale);
      setLanguageSelection(previousPreference ?? "");
      setLanguageFeedback(errorFeedback(error));
    } finally {
      setLanguageSaving(false);
    }
  };

  const changeTheme = async (preference: ThemePreference) => {
    if (themeSaving || resetting || !data.config || preference === themeSelection) return;
    const previousPreference = normalizeThemePreference(data.config.ui.theme);
    setThemeSelection(preference);
    data.setThemePreference(preference);
    setThemeSaving(true);
    setThemeFeedback(null);
    try {
      const result = await data.mutateConfig((latest) => ({
        ...latest,
        ui: { ...latest.ui, theme: preference },
      }));
      if (result.ok) {
        setThemeFeedback({ kind: "success", key: result.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "settings.themeSaved" });
      } else {
        data.setThemePreference(previousPreference);
        setThemeSelection(previousPreference);
        setThemeFeedback(saveError(result));
      }
    } catch (error) {
      data.setThemePreference(previousPreference);
      setThemeSelection(previousPreference);
      setThemeFeedback(errorFeedback(error));
    } finally {
      setThemeSaving(false);
    }
  };

  const changeDesktopPreferences = async (patch: DesktopPreferencesPatch) => {
    if (notificationSaving || resetting || !data.config) return;
    setNotificationSaving(true);
    setNotificationFeedback(null);
    try {
      const result = await data.setDesktopPreferences(patch);
      setNotificationFeedback(result.ok
        ? { kind: "success", key: "settings.notifications.saved" }
        : saveError(result));
    } catch (error) {
      setNotificationFeedback(errorFeedback(error));
    } finally {
      setNotificationSaving(false);
    }
  };

  const chooseFolder = async () => {
    if (workspaceSaving || resetting || !data.config) return;
    const path = await data.chooseWorkspace();
    if (!path) return;
    setWorkspaceSaving(true);
    setWorkspaceFeedback(null);
    try {
      const result = await data.mutateConfig((latest) => ({ ...latest, workspace: { rootPath: path } }));
      setWorkspaceFeedback(result.ok
        ? { kind: "success", key: result.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved" }
        : saveError(result));
    } catch (error) {
      setWorkspaceFeedback(errorFeedback(error));
    } finally {
      setWorkspaceSaving(false);
    }
  };

  const openConnection = () => {
    if (!data.config || connectionSaving || resetting) return;
    setConnectionObserved({ ...data.config.tunnel });
    setConnectionFeedback(null);
    setConnectionDirty(false);
    setConnectionOpen(true);
  };

  const applyConnection = async (draft: TunnelDraft, apiKey: string, observed: TunnelConfig): Promise<SaveResult> => {
    const nextProxy = draft.controlPlaneHttpProxy.trim();
    const proxyError = validateControlPlaneProxy(nextProxy);
    if (proxyError) {
      const result: SaveResult = { ok: false, error: undefined };
      setConnectionFeedback({ kind: "error", summaryKey: proxyError });
      return result;
    }
    setConnectionSaving(true);
    setConnectionFeedback(null);
    try {
      const result = await data.mutateConfig((latest) => {
        assertUnchanged(observed, latest.tunnel);
        const tunnel: TunnelConfig = {
          ...latest.tunnel,
          tunnelId: draft.tunnelId.trim(),
          autoDetect: draft.autoDetect,
        };
        if (nextProxy) tunnel.controlPlaneHttpProxy = nextProxy;
        else delete tunnel.controlPlaneHttpProxy;
        if (draft.tunnelClientPath.trim()) tunnel.tunnelClientPath = draft.tunnelClientPath.trim();
        else delete tunnel.tunnelClientPath;
        return { ...latest, tunnel };
      }, apiKey.trim() || undefined);
      if (!result.ok) {
        setConnectionFeedback(saveError(result));
      } else {
        setConnectionFeedback({ kind: "success", key: result.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved" });
      }
      return result;
    } catch (error) {
      const result: SaveResult = { ok: false };
      setConnectionFeedback(errorFeedback(error));
      return result;
    } finally {
      setConnectionSaving(false);
    }
  };

  const finishConnection = () => {
    finishDraft();
    setConnectionOpen(false);
    setConnectionObserved(null);
    setConnectionDirty(false);
  };

  const removeApiKey = () => {
    // A credential removal is independent from the Connection draft. Closing
    // the editor first ensures Cancel can never restore the removed secret.
    if (connectionSaving || resetting) return;
    setConnectionOpen(false);
    setConnectionObserved(null);
    setConnectionDirty(false);
    setConnectionFeedback(null);
    setRemoveConfirmation(true);
  };

  const confirmRemoveApiKey = async () => {
    if (removingApiKey) return;
    setRemovingApiKey(true);
    try {
      await data.deleteApiKey();
      setRemoveConfirmation(false);
      setConnectionFeedback({ kind: "success", key: "settings.tunnel.removed" });
    } catch (error) {
      setConnectionFeedback(errorFeedback(error));
    } finally {
      setRemovingApiKey(false);
    }
  };

  const commitSecurity = async (mode: SecurityMode) => {
    setSecuritySaving(true);
    setSecurityFeedback(null);
    try {
      const result = await data.mutateConfig((latest) => ({
        ...latest,
        security: { ...latest.security, mode, developerTestMode: mode === "developer_test" },
      }));
      if (result.ok) {
        setSecurityFeedback({ kind: "success", key: result.refreshUnconfirmed ? "m8.refreshUnconfirmed" : "m8.saved" });
        setDeveloperConfirmation(false);
      } else {
        setSecurityFeedback(saveError(result));
      }
    } catch (error) {
      setSecurityFeedback(errorFeedback(error));
    } finally {
      setSecuritySaving(false);
    }
  };

  const chooseSecurityMode = (mode: SecurityMode) => {
    if (securitySaving || resetting || !savedMode || mode === savedMode) return;
    requestLeave(() => {
      if (mode === "developer_test") setDeveloperConfirmation(true);
      else void commitSecurity(mode);
    });
  };

  const requestReset = () => requestLeave(() => {
    setResetFeedback(null);
    setResetConfirmation(true);
  });

  const reset = async () => {
    if (resetting || connectionSaving) return;
    setResetFeedback(null);
    setConnectionOpen(false);
    setConnectionObserved(null);
    setConnectionDirty(false);
    setResetting(true);
    try {
      const result = await data.resetConfig();
      if (result.ok) {
        setResetConfirmation(false);
        setWorkspaceFeedback({ kind: "success", key: "settings.actions.resetSuccess" });
      } else {
        const failed = saveError(result);
        setResetFeedback(failed);
        setWorkspaceFeedback(failed);
      }
    } catch (error) {
      const failed = errorFeedback(error);
      setResetFeedback(failed);
      setWorkspaceFeedback(failed);
    } finally {
      setResetting(false);
    }
  };

  const restartHarness = async () => {
    const result = await data.restartHarness();
    setWorkspaceFeedback(result.ok ? { kind: "success", key: "settings.restart.success" } : saveError(result));
  };

  const section = (name: NavigationOptions["settingsSection"], content: ReactNode) => (
    <div id={`settings-section-${name}`} tabIndex={-1}>{content}</div>
  );

  return (
    <div className="settings-page page-stack">
      <p>{t("settings.banner.description")}</p>

      {data.configError && <ConfigRecovery onReset={requestReset} disabled={resetting || connectionSaving} />}

      {section("general", <Section title={t("m8Settings.general")}>
        <Row title={t("settings.language")} summary={t("settings.languageHelp")} controls={<Select label={t("settings.language")} value={languageSelection} onChange={(event) => void changeLanguage(event.target.value as UiLanguage | "")} disabled={languageSaving || resetting}>
          <option value="">{t("settings.followSystem")}</option><option value="en">{t("settings.languages.en")}</option><option value="zh-CN">{t("settings.languages.zhCN")}</option>
        </Select>} />
        {languageSaving && <InlineFeedback>{t("settings.languageSaving")}</InlineFeedback>}
        {languageFeedback && <FeedbackNotice feedback={languageFeedback} />}
      </Section>)}

      {section("appearance", <Section title={t("settings.appearance")}>
        <Row title={t("settings.theme")} summary={t("settings.themeHelp")} controls={<Segmented
          value={themeSelection}
          label={t("settings.theme")}
          disabled={themeSaving || resetting || !data.config}
          options={[
            { value: "system" as const, label: t("settings.themes.system") },
            { value: "light" as const, label: t("settings.themes.light") },
            { value: "dark" as const, label: t("settings.themes.dark") },
          ]}
          onChange={(value) => void changeTheme(value)}
        />} />
        {themeSaving && <InlineFeedback>{t("settings.themeSaving")}</InlineFeedback>}
        {themeFeedback && <FeedbackNotice feedback={themeFeedback} />}
      </Section>)}

      {section("notifications", <Section title={t("settings.notifications.title")}>
        <Row
          title={t("settings.notifications.approvalTitle")}
          summary={t("settings.notifications.approvalHelp")}
          controls={<input
            type="checkbox"
            aria-label={t("settings.notifications.approvalTitle")}
            checked={currentConfig?.desktop.approvalNotifications ?? true}
            onChange={(event) => void changeDesktopPreferences({ approvalNotifications: event.target.checked })}
            disabled={notificationSaving || resetting || !data.config}
          />}
        />
        <Row
          title={t("settings.notifications.connectionTitle")}
          summary={t("settings.notifications.connectionHelp")}
          controls={<input
            type="checkbox"
            aria-label={t("settings.notifications.connectionTitle")}
            checked={currentConfig?.desktop.connectionNotifications ?? true}
            onChange={(event) => void changeDesktopPreferences({ connectionNotifications: event.target.checked })}
            disabled={notificationSaving || resetting || !data.config}
          />}
        />
        <InlineFeedback>{t("settings.notifications.fallback")}</InlineFeedback>
        {notificationFeedback && <FeedbackNotice feedback={notificationFeedback} />}
      </Section>)}

      {section("history", <HistorySettingsSection />)}

      {section("workspace", <Section title={t("m8Settings.workspace")}>
        <Row title={t("settings.workspace.root")} summary={<code className="mono">{currentConfig?.workspace.rootPath || t("m8.unknown")}</code>} controls={<Button variant="secondary" onClick={() => void chooseFolder()} disabled={workspaceSaving || resetting || !data.config}>{workspaceSaving ? t("settings.languageSaving") : t("settings.workspace.choose")}</Button>} />
        <Row title={t("m8.running")} summary={<code className="mono">{observedRuntime?.activeWorkspace || t("m8.unknown")}</code>} />
        <InlineFeedback>{t("settings.workspace.help")}</InlineFeedback>
        {running && data.restartRequired && <ConfigRevisionStatus titleKey="settings.restart.title" descriptionKey="settings.restart.description" onRestart={() => void restartHarness()} />}
        {workspaceFeedback && <FeedbackNotice feedback={workspaceFeedback} />}
      </Section>)}

      {section("connection", <Section title={t("m8Settings.connection")}>
        <Row title={t("settings.tunnel.id")} summary={<code className="mono">{currentConfig?.tunnel.tunnelId || t("m8.unknown")}</code>} controls={<Button variant="secondary" onClick={openConnection} disabled={!data.config || connectionSaving || resetting}>{t("m8Settings.configure")}</Button>} />
        <Row title={t("settings.tunnel.apiKey")} summary={apiKeyStatus === "configured" ? <StatusBadge tone="success">{t("settings.tunnel.configured")}</StatusBadge> : <span className="muted">{apiKeyStatus === "missing" ? t("m8Settings.notConfigured") : t("m8.unknown")}</span>} />
        <Row title={t("settings.tunnel.title")} summary={connectionStatus} />
        {connectionFeedback && <FeedbackNotice feedback={connectionFeedback} />}
      </Section>)}

      {section("security", <Section title={t("m8Settings.security")}>
        <Row title={t("settings.security.title")} summary={<span>{savedMode ? `${t("m8.configured")}: ${t(securityModeKey(savedMode))}` : t("m8.unknown")}</span>} controls={savedMode ? <SegmentedSecurity value={savedMode} onChange={chooseSecurityMode} disabled={securitySaving || resetting} /> : <span className="muted">{t("m8.unknown")}</span>} />
        <InlineFeedback>{observedRuntime ? `${t("m8.running")}: ${t(securityModeKey(observedRuntime.securityMode))}` : `${t("m8.running")}: ${t("m8.unknown")}`}</InlineFeedback>
        {securityFeedback && <FeedbackNotice feedback={securityFeedback} />}
      </Section>)}

      {section("advanced", <Section title={t("m8Settings.advanced")}>
        <Disclosure title={t("settings.localData.title")} open={initialSection === "advanced"}>
          <dl className="detail-list">
            <div><dt>{t("settings.localData.configDirectory")}</dt><dd><code>{data.configMetadata?.configDirectory ?? t("settings.localData.osApplicationData")}</code></dd></div>
            <div><dt>{t("settings.localData.configFile")}</dt><dd><code>{data.configMetadata?.configFile ?? t("settings.localData.configJson")}</code></dd></div>
            <div><dt>{t("settings.localData.secretProvider")}</dt><dd><code>{currentConfig?.tunnel.apiKeyRef?.provider ?? t("m8.unknown")}</code></dd></div>
            <div><dt>{t("m7.revision.title")}</dt><dd><code>{currentConfig?.configRevision ?? t("m8.unknown")}</code></dd></div>
          </dl>
        </Disclosure>
        <DiagnosticsPanel run={data.lastDiagnostics} startup={data.latestStartupAttempt ?? null} onRun={data.runDiagnostics} onNavigate={onNavigate} />
      </Section>)}

      {section("danger", <Section title={t("m8Settings.dangerZone")}>
        <InlineFeedback>{t("m8FirstRun.resetLoss")}</InlineFeedback>
        <Button variant="danger" onClick={requestReset} disabled={resetting || connectionSaving}>{resetting ? t("settings.actions.resetting") : t("settings.actions.reset")}</Button>
      </Section>)}

      {connectionOpen && connectionObserved && <ConnectionEditor
        observed={connectionObserved}
        apiKeyConfigured={data.apiKeyConfigured}
        apiKeyObserved={data.apiKeyObserved}
        latest={data.config?.tunnel ?? null}
        saving={connectionSaving}
        feedback={connectionFeedback}
        onDirtyChange={setConnectionDirty}
        onApply={applyConnection}
        onSaved={finishConnection}
        onCancel={finishConnection}
        onChooseClient={() => data.chooseTunnelClient()}
        onDetectClient={() => data.detectTunnelClient()}
        onRemove={removeApiKey}
        onReload={() => data.config?.tunnel ?? null}
      />}

      {developerConfirmation && <Dialog title={t("settings.security.developerTest")} onClose={() => { if (!securitySaving) setDeveloperConfirmation(false); }}>
        <p>{t("settings.security.developerDescription")}</p>
        <p className="muted">{t("settings.security.normalDescription")}</p>
        {securityFeedback && <FeedbackNotice feedback={securityFeedback} />}
        <div className="settings-actions"><Button variant="secondary" onClick={() => setDeveloperConfirmation(false)} disabled={securitySaving}>{t("common.cancel")}</Button><Button variant="primary" onClick={() => void commitSecurity("developer_test")} disabled={securitySaving}>{t("settings.security.developerTest")}</Button></div>
      </Dialog>}

      {resetConfirmation && <Dialog title={t("settings.actions.reset")} onClose={() => { if (!resetting) setResetConfirmation(false); }}>
        <p>{t("m8FirstRun.resetLoss")}</p>
        {resetFeedback && <FeedbackNotice feedback={resetFeedback} />}
        <div className="settings-actions"><Button variant="secondary" onClick={() => setResetConfirmation(false)} disabled={resetting}>{t("common.cancel")}</Button><Button variant="danger" onClick={() => void reset()} disabled={resetting || connectionSaving}>{resetting ? t("settings.actions.resetting") : t("settings.actions.reset")}</Button></div>
      </Dialog>}

      {removeConfirmation && <Dialog title={t("settings.tunnel.remove")} onClose={() => { if (!removingApiKey) setRemoveConfirmation(false); }}>
        <p>{t("settings.tunnel.removeConfirm")}</p>
        <p className="muted">{t("settings.tunnel.configured")}</p>
        {connectionFeedback && <FeedbackNotice feedback={connectionFeedback} />}
        <div className="settings-actions"><Button variant="secondary" onClick={() => setRemoveConfirmation(false)} disabled={removingApiKey}>{t("common.cancel")}</Button><Button variant="danger" onClick={() => void confirmRemoveApiKey()} disabled={removingApiKey}>{removingApiKey ? t("common.saving") : t("settings.tunnel.remove")}</Button></div>
      </Dialog>}
    </div>
  );
}

function SegmentedSecurity({ value, onChange, disabled }: { value: SecurityMode; onChange: (value: SecurityMode) => void; disabled?: boolean }) {
  const { t } = useTranslation();
  return <Segmented
    value={value}
    label={t("settings.security.title")}
    disabled={disabled}
    options={[
      { value: "normal" as const, label: t("settings.security.normal") },
      { value: "developer_test" as const, label: t("settings.security.developerTest") },
    ]}
    onChange={onChange}
  />;
}

function ConnectionEditor({
  observed,
  latest,
  apiKeyConfigured,
  apiKeyObserved,
  saving,
  feedback,
  onDirtyChange,
  onApply,
  onSaved,
  onCancel,
  onChooseClient,
  onDetectClient,
  onRemove,
  onReload,
}: {
  observed: TunnelConfig;
  latest: TunnelConfig | null;
  apiKeyConfigured: boolean;
  apiKeyObserved: boolean;
  saving: boolean;
  feedback: Feedback | null;
  onDirtyChange: (dirty: boolean) => void;
  onApply: (draft: TunnelDraft, apiKey: string, observed: TunnelConfig) => Promise<SaveResult>;
  onSaved: () => void;
  onCancel: () => void;
  onChooseClient: () => Promise<string | null>;
  onDetectClient: () => Promise<string | null>;
  onRemove: () => void;
  onReload: () => TunnelConfig | null;
}) {
  const { t } = useTranslation();
  const [baseline, setBaseline] = useState<TunnelConfig>(() => observed);
  const [draft, setDraft] = useState<TunnelDraft>(() => tunnelDraft(observed));
  const [apiKey, setApiKey] = useState("");
  const [replacing, setReplacing] = useState(apiKeyObserved && !apiKeyConfigured);
  const [replacementRequired, setReplacementRequired] = useState(false);
  const [apiKeyError, setApiKeyError] = useState(false);
  const [proxyError, setProxyError] = useState<ProxyValidationKey>("");
  const conflict = latest !== null && JSON.stringify(latest) !== JSON.stringify(baseline);
  const unconfirmed = latest === null;
  const dirty = JSON.stringify(draft) !== JSON.stringify(tunnelDraft(baseline)) || Boolean(apiKey);
  const requestLeave = useDraftGuard({
    dirty,
    saving,
    onDiscard: () => { setDraft(tunnelDraft(baseline)); setApiKey(""); setReplacementRequired(false); setApiKeyError(false); },
  });

  useEffect(() => { onDirtyChange(dirty); }, [dirty, onDirtyChange]);

  const close = () => requestLeave(onCancel);
  const setField = <K extends keyof TunnelDraft>(key: K, value: TunnelDraft[K]) => {
    setDraft((current) => ({ ...current, [key]: value }));
    if (key === "controlPlaneHttpProxy") setProxyError("");
  };
  const apply = async () => {
    const error = validateControlPlaneProxy(draft.controlPlaneHttpProxy.trim());
    setProxyError(error);
    if (error) return;
    if (replacementRequired && !apiKey.trim()) {
      setApiKeyError(true);
      return;
    }
    if (conflict || unconfirmed) return;
    const result = await onApply(draft, apiKey, baseline);
    // API key input is controlled and ephemeral even when the cross-store save
    // reports a partial failure. A retry must ask for the secret again.
    setApiKey("");
    if (result.ok) {
      setReplacementRequired(false);
      setApiKeyError(false);
      onSaved();
    } else if (apiKey.trim()) {
      setReplacementRequired(true);
      setApiKeyError(true);
    }
  };
  const reload = () => {
    const next = onReload();
    if (!next) return;
    setBaseline(next);
    setDraft(tunnelDraft(next));
    setApiKey("");
    setProxyError("");
    setReplacementRequired(false);
    setApiKeyError(false);
    setReplacing(apiKeyObserved && !apiKeyConfigured);
  };
  const remove = () => requestLeave(() => { setApiKey(""); onRemove(); });
  const chooseClient = async () => {
    const path = await onChooseClient();
    if (path) setDraft((current) => ({ ...current, tunnelClientPath: path, autoDetect: false }));
  };
  const detectClient = async () => {
    const path = await onDetectClient();
    if (path) setDraft((current) => ({ ...current, tunnelClientPath: path, autoDetect: true }));
  };

  return <Sheet title={t("m8Settings.connection")} onClose={close}>
    <div className="page-stack">
      <TextField label={t("settings.tunnel.id")} value={draft.tunnelId} onChange={(event) => setField("tunnelId", event.target.value)} disabled={saving} placeholder={t("settings.tunnel.idPlaceholder")} />
      <div className="form-field">
        <span>{t("settings.tunnel.apiKey")}</span>
        {apiKeyObserved && apiKeyConfigured && !replacing ? <div className="secret-state">
          <StatusBadge tone="success">{t("settings.tunnel.configured")}</StatusBadge>
          <Button variant="quiet" onClick={() => { setReplacing(true); setApiKey(""); }} disabled={saving}>{t("settings.tunnel.replace")}</Button>
          <Button variant="quiet" className="danger-text" onClick={remove} disabled={saving}>{t("settings.tunnel.remove")}</Button>
        </div> : (replacing || apiKeyObserved) ? <TextField label={t("settings.tunnel.apiKey")} type="password" value={apiKey} onChange={(event) => { setApiKey(event.target.value); setApiKeyError(false); }} disabled={saving} placeholder={t("settings.tunnel.apiKeyPlaceholder")} autoComplete="off" /> : <div className="secret-state">
          <StatusBadge tone="neutral">{t("m8.unknown")}</StatusBadge>
          <Button variant="quiet" onClick={() => { setReplacing(true); setApiKey(""); }} disabled={saving}>{t("settings.tunnel.replace")}</Button>
        </div>}
        {apiKeyError && <InlineFeedback error>{t("m8FirstRun.keyRequired")}</InlineFeedback>}
      </div>
      <Disclosure title={t("m8.advanced")}>
        <TextField label={t("settings.tunnel.proxy")} value={draft.controlPlaneHttpProxy} onChange={(event) => setField("controlPlaneHttpProxy", event.target.value)} disabled={saving} placeholder={t("settings.tunnel.proxyPlaceholder")} error={proxyError ? t(proxyError) : undefined} helper={t("settings.tunnel.proxyHelp")} />
        <div className="form-field"><span>{t("settings.tunnel.client")}</span><div className="form-row">
          <input className="path-input grow" aria-label={t("settings.tunnel.client")} value={draft.tunnelClientPath} readOnly placeholder={t("settings.tunnel.clientPlaceholder")} title={draft.tunnelClientPath || undefined} />
          <Button variant="secondary" onClick={() => void detectClient()} disabled={saving}>{t("settings.tunnel.useBundled")}</Button>
          <Button variant="secondary" onClick={() => void chooseClient()} disabled={saving}>{t("settings.tunnel.chooseOverride")}</Button>
        </div></div>
      </Disclosure>
      {saving && <InlineFeedback>{t("common.saving")}</InlineFeedback>}
      {conflict && <InlineFeedback error>{t("m8.conflict")} <Button variant="quiet" onClick={reload} disabled={saving}>{t("m8.reload")}</Button></InlineFeedback>}
      {feedback && <FeedbackNotice feedback={feedback} />}
      {unconfirmed && <InlineFeedback error>{t("m8.unknown")}</InlineFeedback>}
      <div className="settings-actions"><Button variant="secondary" onClick={close} disabled={saving}>{t("common.cancel")}</Button><Button variant="primary" onClick={() => void apply()} disabled={saving || !dirty || conflict || unconfirmed}>{t("m8Settings.apply")}</Button></div>
    </div>
  </Sheet>;
}

export { FirstRunWizard } from "./FirstRunWizard";

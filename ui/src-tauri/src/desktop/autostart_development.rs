//! Isolated development tests of the real Host Start path with fixture children.
//! They do not contact a control plane, modify user credentials, or claim E2E.

use super::{
    autostart::{self, AutoStartCoordinator, Outcome},
    DesktopLifecycleCoordinator,
};
use crate::{
    commands::BackendState,
    config::ConfigStore,
    harness::{HarnessController, HarnessLifecycle},
    runtime::{RuntimeProcessPhase, RuntimeSupervisor},
    secrets::{MemorySecretStore, SecretStore},
    tunnel::TunnelProcessManager,
};
use harness_control_protocol::{SecretReference, StartupOutcome};
use std::{
    env, fs,
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

struct Fixture {
    app: AppHandle,
    worker: Option<thread::JoinHandle<()>>,
    coordinator: AutoStartCoordinator,
    _root: tempfile::TempDir,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self
            .app
            .state::<HarnessController>()
            .stop_for_quit(&self.app);
        self.app.exit(0);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn wait_for(mut predicate: impl FnMut() -> bool, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(Instant::now() < deadline, "{message}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn fixture(scenario: &str) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let tunnel_dir = root.path().join("tunnel");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir(&tunnel_dir).unwrap();
    fs::write(tunnel_dir.join("tunnel-client.exe"), b"fixture").unwrap();
    fs::write(tunnel_dir.join("cloudflared.exe"), b"fixture").unwrap();
    let mut config = ConfigStore::safe_defaults();
    config.workspace.root_path = workspace.to_string_lossy().into_owned();
    config.tunnel.tunnel_id = "m07-test-tunnel".into();
    config.tunnel.tunnel_client_path = Some(tunnel_dir.to_string_lossy().into_owned());
    config.tunnel.api_key_ref = Some(SecretReference {
        provider: "memory-development".into(),
        key: "m07-test-key".into(),
    });
    if scenario == "bad_workspace" {
        config.workspace.root_path = root
            .path()
            .join("missing-workspace")
            .to_string_lossy()
            .into_owned();
    }
    if scenario == "missing_tunnel" {
        config.tunnel.tunnel_id.clear();
    }
    if scenario == "missing_reference" {
        config.tunnel.api_key_ref = None;
    }
    if scenario == "bad_tunnel" {
        config.tunnel.tunnel_client_path = Some(
            root.path()
                .join("missing-tunnel")
                .to_string_lossy()
                .into_owned(),
        );
    }
    config.desktop.auto_start_harness_on_app_launch = scenario != "disabled";
    ConfigStore::new(root.path().to_path_buf())
        .save(config)
        .unwrap();
    if scenario == "invalid_config" {
        fs::write(root.path().join("config.json"), b"{invalid").unwrap();
    }

    let secrets = MemorySecretStore::default();
    if scenario != "missing_secret" {
        secrets.set("m07-test-key", "synthetic-test-value").unwrap();
    }
    let slow = matches!(scenario, "auto_first" | "manual_first" | "quit_during");
    let runtime = crate::runtime::tests::test_supervisor(
        if slow { "slow_initialize" } else { "normal" },
        Duration::from_secs(2),
    );
    let tunnel = crate::tunnel::tests::sleeping_manager();
    let harness = HarnessController::new(runtime.clone(), tunnel.clone());
    runtime.set_event_sink(Arc::new(harness.clone()));
    runtime.set_startup_sink(Arc::new(harness.clone()));
    tunnel.set_event_sink(Arc::new(harness.clone()));
    let desktop = DesktopLifecycleCoordinator::new(harness.clone());
    let desktop_for_status = desktop.clone();
    harness.set_status_sink(Arc::new(move |status| {
        desktop_for_status.observe_status(status.clone());
    }));
    let coordinator = AutoStartCoordinator::default();
    let setup_coordinator = coordinator.clone();
    let root_path = root.path().to_path_buf();
    let manual_schedule = matches!(scenario, "manual_first" | "quit_before" | "quit_cancelled");
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut context = tauri::generate_context!();
        context.config_mut().identifier =
            format!("com.chatgpt.harness.m07.dev.{}", uuid::Uuid::new_v4());
        context.config_mut().app.windows.clear();
        context.config_mut().app.app_directories_override = Some(
            tauri::utils::config::AppDirectoriesOverride::Root(root_path),
        );
        let app = tauri::Builder::<tauri::Wry>::default()
            .any_thread()
            .plugin(super::single_instance::plugin())
            .manage(runtime)
            .manage(tunnel)
            .manage(harness)
            .manage(desktop)
            .manage(Mutex::new(BackendState {
                secrets: Box::new(secrets),
            }))
            .setup(move |app| {
                if !manual_schedule {
                    autostart::schedule(app.handle(), &setup_coordinator);
                }
                sender.send(app.handle().clone()).unwrap();
                Ok(())
            })
            .build(context)
            .expect("isolated development Host");
        let _ = app.run_return(|_, _| {});
    });
    let app = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("Host setup");
    Fixture {
        app,
        worker: Some(worker),
        coordinator,
        _root: root,
    }
}

fn start(
    fixture: &Fixture,
) -> Result<crate::harness::HarnessStatus, crate::commands::ControlError> {
    fixture
        .app
        .state::<DesktopLifecycleCoordinator>()
        .dispatch_start(&fixture.app, &fixture.app.state::<Mutex<BackendState>>())
}

fn ready(fixture: &Fixture) {
    wait_for(
        || fixture.app.state::<HarnessController>().status().lifecycle == HarnessLifecycle::Ready,
        "Harness did not become Ready",
    );
}

fn stopped(fixture: &Fixture) {
    fixture
        .app
        .state::<DesktopLifecycleCoordinator>()
        .dispatch_stop(&fixture.app)
        .unwrap();
    assert_eq!(
        fixture.app.state::<HarnessController>().status().lifecycle,
        HarnessLifecycle::Stopped
    );
    assert_eq!(
        fixture
            .app
            .state::<RuntimeSupervisor>()
            .process_status()
            .phase,
        RuntimeProcessPhase::Stopped
    );
    assert!(!fixture
        .app
        .state::<TunnelProcessManager>()
        .test_has_live_process());
}

fn run_scenario(scenario: &str) {
    let fixture = fixture(scenario);
    let controller = fixture.app.state::<HarnessController>();
    let desktop = fixture.app.state::<DesktopLifecycleCoordinator>();
    match scenario {
        "enabled" => {
            ready(&fixture);
            let first = controller
                .latest_startup_attempt()
                .unwrap()
                .harness_session_id;
            stopped(&fixture);
            autostart::schedule(&fixture.app, &fixture.coordinator);
            thread::sleep(Duration::from_millis(100));
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Stopped);
            assert_eq!(
                controller
                    .latest_startup_attempt()
                    .unwrap()
                    .harness_session_id,
                first
            );
            controller
                .restart(&fixture.app, &fixture.app.state::<Mutex<BackendState>>())
                .unwrap();
            ready(&fixture);
            stopped(&fixture);
        }
        "disabled" => {
            assert_eq!(fixture.coordinator.outcome(), Outcome::Disabled);
            assert!(controller.latest_startup_attempt().is_none());
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Stopped);
            start(&fixture).unwrap();
            ready(&fixture);
            stopped(&fixture);
        }
        "missing_secret" => {
            wait_for(
                || fixture.coordinator.outcome() == Outcome::Failed,
                "missing secret did not fail",
            );
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Error);
            let attempt = controller.latest_startup_attempt().unwrap();
            assert_eq!(attempt.outcome, StartupOutcome::Failed);
            let backend = fixture.app.state::<Mutex<BackendState>>();
            let runtime = fixture.app.state::<RuntimeSupervisor>();
            let tunnel = fixture.app.state::<TunnelProcessManager>();
            let diagnostics = crate::diagnostics::DiagnosticsCoordinator::new(
                &fixture.app,
                &backend,
                &runtime,
                &tunnel,
                &controller,
            )
            .run();
            assert_eq!(
                diagnostics
                    .checks
                    .iter()
                    .find(|check| check.id == "credential_presence")
                    .unwrap()
                    .status,
                harness_control_protocol::DiagnosticStatus::Fail,
            );
            assert_eq!(
                controller
                    .latest_startup_attempt()
                    .unwrap()
                    .harness_session_id,
                attempt.harness_session_id
            );
            fixture
                .app
                .state::<Mutex<BackendState>>()
                .lock()
                .unwrap()
                .secrets
                .set("m07-test-key", "synthetic-test-value")
                .unwrap();
            autostart::schedule(&fixture.app, &fixture.coordinator);
            thread::sleep(Duration::from_millis(100));
            assert_eq!(
                controller
                    .latest_startup_attempt()
                    .unwrap()
                    .harness_session_id,
                attempt.harness_session_id
            );
            start(&fixture).unwrap();
            ready(&fixture);
            stopped(&fixture);
        }
        "bad_workspace" | "missing_tunnel" | "missing_reference" | "bad_tunnel" => {
            wait_for(
                || fixture.coordinator.outcome() == Outcome::Failed,
                "invalid eligibility did not fail",
            );
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Error);
            assert_eq!(
                controller.latest_startup_attempt().unwrap().outcome,
                StartupOutcome::Failed
            );
            assert_eq!(
                fixture
                    .app
                    .state::<RuntimeSupervisor>()
                    .process_status()
                    .phase,
                RuntimeProcessPhase::Stopped
            );
            assert!(!fixture
                .app
                .state::<TunnelProcessManager>()
                .test_has_live_process());
        }
        "invalid_config" => {
            assert_eq!(fixture.coordinator.outcome(), Outcome::Failed);
            assert!(controller.latest_startup_attempt().is_none());
            assert_eq!(
                fs::read(fixture._root.path().join("config.json")).unwrap(),
                b"{invalid"
            );
        }
        "auto_first" => {
            wait_for(
                || {
                    fixture
                        .app
                        .state::<RuntimeSupervisor>()
                        .process_status()
                        .phase
                        == RuntimeProcessPhase::Starting
                },
                "automatic Start did not enter Runtime startup",
            );
            assert_eq!(start(&fixture).unwrap_err().code, "LIFECYCLE_CONFLICT");
            ready(&fixture);
            stopped(&fixture);
        }
        "manual_first" => {
            let app = fixture.app.clone();
            let worker = thread::spawn(move || {
                app.state::<DesktopLifecycleCoordinator>()
                    .dispatch_start(&app, &app.state::<Mutex<BackendState>>())
            });
            wait_for(
                || {
                    fixture
                        .app
                        .state::<RuntimeSupervisor>()
                        .process_status()
                        .phase
                        == RuntimeProcessPhase::Starting
                },
                "manual Start did not enter Runtime startup",
            );
            autostart::schedule(&fixture.app, &fixture.coordinator);
            wait_for(
                || fixture.coordinator.outcome() == Outcome::Skipped,
                "automatic Start was not skipped",
            );
            worker.join().unwrap().unwrap();
            ready(&fixture);
            stopped(&fixture);
        }
        "quit_before" => {
            assert!(desktop.begin_quit());
            autostart::schedule(&fixture.app, &fixture.coordinator);
            wait_for(
                || fixture.coordinator.outcome() == Outcome::Skipped,
                "quit did not block automatic Start",
            );
            assert!(controller.latest_startup_attempt().is_none());
            assert_eq!(
                fixture
                    .app
                    .state::<RuntimeSupervisor>()
                    .process_status()
                    .phase,
                RuntimeProcessPhase::Stopped
            );
        }
        "quit_cancelled" => {
            assert!(desktop.begin_quit());
            desktop.cancel_quit();
            autostart::schedule(&fixture.app, &fixture.coordinator);
            wait_for(
                || fixture.coordinator.outcome() == Outcome::Skipped,
                "failed quit rearmed automatic Start",
            );
            assert!(controller.latest_startup_attempt().is_none());
            assert_eq!(
                fixture
                    .app
                    .state::<RuntimeSupervisor>()
                    .process_status()
                    .phase,
                RuntimeProcessPhase::Stopped
            );
            start(&fixture).unwrap();
            ready(&fixture);
            stopped(&fixture);
        }
        "quit_during" => {
            wait_for(
                || {
                    fixture
                        .app
                        .state::<RuntimeSupervisor>()
                        .process_status()
                        .phase
                        == RuntimeProcessPhase::Starting
                },
                "automatic Start did not begin",
            );
            assert!(desktop.begin_quit());
            controller.stop_for_quit(&fixture.app).unwrap();
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Stopped);
            assert_eq!(
                fixture
                    .app
                    .state::<RuntimeSupervisor>()
                    .process_status()
                    .phase,
                RuntimeProcessPhase::Stopped
            );
            assert!(!fixture
                .app
                .state::<TunnelProcessManager>()
                .test_has_live_process());
            autostart::schedule(&fixture.app, &fixture.coordinator);
            thread::sleep(Duration::from_millis(100));
            assert_eq!(controller.status().lifecycle, HarnessLifecycle::Stopped);
        }
        _ => panic!("unknown scenario"),
    }
}

#[test]
fn isolated_scenario_process() {
    if let Ok(scenario) = env::var("M07_DEVELOPMENT_SCENARIO") {
        run_scenario(&scenario);
    }
}

#[test]
fn isolated_host_development_scenarios() {
    for scenario in [
        "enabled",
        "disabled",
        "missing_secret",
        "invalid_config",
        "bad_workspace",
        "missing_tunnel",
        "missing_reference",
        "bad_tunnel",
        "auto_first",
        "manual_first",
        "quit_before",
        "quit_cancelled",
        "quit_during",
    ] {
        let mut child = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "desktop::autostart_development::isolated_scenario_process",
                "--nocapture",
            ])
            .env("M07_DEVELOPMENT_SCENARIO", scenario)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(25);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{scenario}: timed out");
            }
            thread::sleep(Duration::from_millis(20));
        };
        if !status.success() {
            use std::io::Read;
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!("{scenario}: {status}: {stderr}");
        }
    }
}

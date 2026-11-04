use super::{routing, DesktopLifecycleCoordinator};
use tauri::Manager;

/// A second invocation activates the existing app; arguments are not used or logged.
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if app
            .try_state::<DesktopLifecycleCoordinator>()
            .is_some_and(|desktop| !desktop.snapshot().state.app_quitting)
        {
            let _ = routing::show_main_window(app, None);
        }
    })
}

#[cfg(all(test, feature = "native-tray-tests"))]
mod tests {
    use std::{
        env, fs,
        process::{Child, Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn isolated_child() {
        let Ok(root) = env::var("M07_SINGLE_INSTANCE_ROOT") else {
            return;
        };
        let identifier = env::var("M07_SINGLE_INSTANCE_ID").unwrap();
        let mut context = tauri::generate_context!();
        context.config_mut().identifier = identifier;
        context.config_mut().app.windows.clear();
        context.config_mut().app.app_directories_override = Some(
            tauri::utils::config::AppDirectoriesOverride::Root(root.clone().into()),
        );
        let runtime = crate::runtime::RuntimeSupervisor::new();
        let tunnel = crate::tunnel::TunnelProcessManager::new();
        let harness = crate::harness::HarnessController::new(runtime, tunnel);
        let desktop = super::DesktopLifecycleCoordinator::new(harness);
        let ready = std::path::Path::new(&root).join("ready");
        let release = std::path::Path::new(&root).join("release");
        let app = tauri::Builder::<tauri::Wry>::default()
            .any_thread()
            .plugin(super::plugin())
            .manage(desktop)
            .setup(move |app| {
                fs::write(ready, b"ready")?;
                let handle = app.handle().clone();
                thread::spawn(move || {
                    while !release.exists() {
                        thread::sleep(Duration::from_millis(20));
                    }
                    handle.exit(0);
                });
                Ok(())
            })
            .build(context)
            .expect("build isolated Host");
        let _ = app.run_return(|_, _| {});
    }

    struct Children(Vec<Child>);
    impl Drop for Children {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[test]
    fn duplicate_process_exits_before_host_setup() {
        let root = tempfile::tempdir().unwrap();
        let identifier = format!("com.chatgpt.harness.m07.{}", uuid::Uuid::new_v4());
        let executable = env::current_exe().unwrap();
        let spawn = || {
            Command::new(&executable)
                .args([
                    "--exact",
                    "desktop::single_instance::tests::isolated_child",
                    "--nocapture",
                ])
                .env("M07_SINGLE_INSTANCE_ROOT", root.path())
                .env("M07_SINGLE_INSTANCE_ID", &identifier)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        };
        let mut children = Children(vec![spawn()]);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.path().join("ready").exists() {
            assert!(
                children.0[0].try_wait().unwrap().is_none(),
                "first Host exited before setup"
            );
            assert!(Instant::now() < deadline, "first Host did not initialize");
            thread::sleep(Duration::from_millis(20));
        }
        let before = fs::metadata(root.path().join("ready"))
            .unwrap()
            .modified()
            .unwrap();
        children.0.push(spawn());
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = children.0[1].try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "second Host did not exit");
            thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success());
        assert!(children.0[0].try_wait().unwrap().is_none());
        assert_eq!(
            fs::metadata(root.path().join("ready"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        fs::write(root.path().join("release"), b"done").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while children.0[0].try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "first Host did not exit");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

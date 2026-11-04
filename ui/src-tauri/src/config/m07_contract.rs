use super::{ConfigStore, DesktopPreferencesPatch, CURRENT_SCHEMA_VERSION};
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};

fn patch(
    auto: Option<bool>,
    approval: Option<bool>,
    connection: Option<bool>,
    notice: Option<bool>,
) -> DesktopPreferencesPatch {
    DesktopPreferencesPatch {
        auto_start_harness_on_app_launch: auto,
        approval_notifications: approval,
        connection_notifications: connection,
        background_notice_acknowledged: notice,
    }
}

#[test]
fn defaults_round_trip_and_focused_auto_start_are_revision_neutral() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    assert_eq!(CURRENT_SCHEMA_VERSION, 3);
    assert!(
        !ConfigStore::safe_defaults()
            .desktop
            .auto_start_harness_on_app_launch
    );
    assert!(store.inspect_read_only().unwrap().is_none());
    let first = store.load_or_create_defaults().unwrap();
    assert!(!first.desktop.auto_start_harness_on_app_launch);
    let initial = store
        .set_desktop_preferences(patch(None, Some(false), Some(false), Some(true)))
        .unwrap();
    let enabled = store
        .set_desktop_preferences(patch(Some(true), None, None, None))
        .unwrap();
    let mut expected = initial.clone();
    expected.desktop.auto_start_harness_on_app_launch = true;
    assert_eq!(enabled, expected);
    assert_eq!(enabled.config_revision, first.config_revision);
    assert_eq!(store.load().unwrap().unwrap(), enabled);
    let disabled = store
        .set_desktop_preferences(patch(Some(false), None, None, None))
        .unwrap();
    assert_eq!(disabled, initial);
    assert_eq!(store.load().unwrap().unwrap(), initial);
    assert!(!directory.path().join("config.json.tmp").exists());
    store.reset().unwrap();
    assert!(
        !store
            .load_or_create_defaults()
            .unwrap()
            .desktop
            .auto_start_harness_on_app_launch
    );
}

#[test]
fn simultaneous_focused_preferences_do_not_lose_each_other() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    for _ in 0..12 {
        store
            .set_desktop_preferences(patch(Some(false), Some(true), Some(true), Some(false)))
            .unwrap();
        let gate = Arc::new(Barrier::new(3));
        let first = store.clone();
        let first_gate = gate.clone();
        let a = thread::spawn(move || {
            first_gate.wait();
            first
                .set_desktop_preferences(patch(Some(true), None, None, None))
                .unwrap();
        });
        let second = store.clone();
        let second_gate = gate.clone();
        let b = thread::spawn(move || {
            second_gate.wait();
            second
                .set_desktop_preferences(patch(None, Some(false), Some(false), Some(true)))
                .unwrap();
        });
        gate.wait();
        a.join().unwrap();
        b.join().unwrap();
        let result = store.load().unwrap().unwrap();
        assert!(result.desktop.auto_start_harness_on_app_launch);
        assert!(!result.desktop.approval_notifications);
        assert!(!result.desktop.connection_notifications);
        assert!(result.desktop.background_notice_acknowledged);
    }
    assert!(!directory.path().join("config.json.tmp").exists());
}

#[test]
fn invalid_config_is_never_treated_as_disabled_or_rewritten() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().into());
    let path = directory.path().join("config.json");
    fs::write(&path, b"{invalid").unwrap();
    assert!(store.inspect_read_only().is_err());
    assert!(store
        .set_desktop_preferences(patch(Some(true), None, None, None))
        .is_err());
    assert_eq!(fs::read(path).unwrap(), b"{invalid");
}

//! Frozen M0 Runtime wire method and notification names.

pub const INITIALIZE: &str = "initialize";
pub const RUNTIME_STATUS: &str = "runtime.status";
pub const RUNTIME_STOP: &str = "runtime.stop";
pub const TOOLS_LIST: &str = "tools.list";
pub const POLICIES_LIST: &str = "policies.list";
pub const ACTIVITY_LIST: &str = "activity.list";
pub const ACTIVITY_SUBSCRIBE: &str = "activity.subscribe";
pub const APPROVALS_LIST: &str = "approvals.list";
pub const APPROVALS_RESOLVE: &str = "approvals.resolve";
pub const DIAGNOSTICS_RUN: &str = "diagnostics.run";

pub const INITIALIZED: &str = "initialized";
pub const ACTIVITY_EVENT: &str = "activity.event";

pub const REQUEST_METHODS: &[&str] = &[
    INITIALIZE,
    RUNTIME_STATUS,
    RUNTIME_STOP,
    TOOLS_LIST,
    POLICIES_LIST,
    ACTIVITY_LIST,
    ACTIVITY_SUBSCRIBE,
    APPROVALS_LIST,
    APPROVALS_RESOLVE,
    DIAGNOSTICS_RUN,
];

pub const NOTIFICATION_METHODS: &[&str] = &[INITIALIZED, ACTIVITY_EVENT];

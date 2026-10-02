/// Internal lifecycle only: deliberately not a second wire-protocol enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestState {
    Received,
    Validating,
    WorkspaceValidating,
    PolicyEvaluating,
    Denied,
    AwaitingApproval,
    Executing,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransitionError {
    pub(crate) from: RequestState,
    pub(crate) to: RequestState,
}

/// The sole state writer; rejected transitions leave state unchanged.
#[derive(Debug)]
pub(super) struct RequestLifecycle(RequestState);

impl RequestLifecycle {
    pub(super) fn new() -> Self {
        Self(RequestState::Received)
    }

    pub(super) fn state(&self) -> RequestState {
        self.0
    }

    pub(super) fn transition(&mut self, next: RequestState) -> Result<(), TransitionError> {
        use RequestState::*;
        if !matches!(
            (self.0, next),
            (Received, Validating)
                | (Validating, WorkspaceValidating | Failed)
                | (WorkspaceValidating, PolicyEvaluating | Failed)
                | (
                    PolicyEvaluating,
                    Denied | AwaitingApproval | Executing | Failed
                )
                | (AwaitingApproval, Denied | Executing | Failed)
                | (Executing, Completed | Failed)
        ) {
            return Err(TransitionError {
                from: self.0,
                to: next,
            });
        }
        self.0 = next;
        Ok(())
    }
}

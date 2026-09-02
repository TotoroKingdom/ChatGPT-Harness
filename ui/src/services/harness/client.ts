import type {
  ActivityEvent,
  ActivityListParams,
  ActivityListResult,
  ActivitySubscribeParams,
  ActivitySubscribeResult,
  ApprovalDecision,
  ApprovalsListParams,
  ApprovalsListResult,
  ApprovalsResolveResult,
  DiagnosticRun,
  PoliciesListResult,
  RuntimeState,
  RuntimeStopResult,
  ToolsListResult,
} from "@protocol/v1";

export type ActivityEventListener = (event: ActivityEvent) => void;
export type Unsubscribe = () => void;

export interface HarnessControlClient {
  runtimeStart(): Promise<RuntimeState>;
  runtimeStatus(): Promise<RuntimeState>;
  runtimeStop(): Promise<RuntimeStopResult>;
  toolsList(): Promise<ToolsListResult>;
  policiesList(): Promise<PoliciesListResult>;
  activityList(params?: ActivityListParams): Promise<ActivityListResult>;
  activitySubscribe(
    listener: ActivityEventListener,
    params?: ActivitySubscribeParams,
  ): Promise<Unsubscribe>;
  approvalsList(params?: ApprovalsListParams): Promise<ApprovalsListResult>;
  approvalsResolve(
    requestId: string,
    decision: ApprovalDecision,
  ): Promise<ApprovalsResolveResult>;
  diagnosticsRun(): Promise<DiagnosticRun>;
}

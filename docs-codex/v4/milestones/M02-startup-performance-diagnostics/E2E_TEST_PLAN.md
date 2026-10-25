# M02 E2E Test Plan — Packaged Ready Correctness and Diagnostics

> 存放规则遵守 AGENTS.md。本目录只保留六份正式文档；执行日志和临时数据使用既有 evidence channel/system temp。  
> Status: VALIDATION BASELINE FROZEN / NOT EXECUTED  
> Milestone: M02 — Startup Performance and One-click Diagnostics  
> Inputs: SPEC.md, PLAN.md, TASKS.md, INTEGRATION_TEST_PLAN.md  
> Owner: QA plus explicitly authorized user/external execution where required  
> Purpose: prove the packaged candidate reports Ready only after the real ChatGPT → Tunnel → Runtime chain is usable and that one-click diagnostics reflects the current external boundary.

## 1. Gate and Safety

Formal E2E requires:

- M01 user acceptance/freeze;
- M02 Development Complete;
- M02 Integration PASS;
- exact packaged candidate and manifest hash fixed;
- controlled Workspace/config/credential and approved network route;
- explicit authorization for any real external ChatGPT call.

Do not substitute a local adapter, direct Runtime request, mocked ChatGPT, process-exists signal or cached readiness and call it E2E PASS.

## 2. Evidence Header

~~~text
Milestone:
Candidate commit/package/hash:
Package type:
Windows build/power mode:
App/Runtime/Tunnel versions:
Workspace fixture identity:
Config revision:
Proxy source/route mode:
Tunnel ID identity (non-secret):
Network access method:
Execution time:
QA owner:
Cleanup:
~~~

Never record the API key, SecretStore value, physical Workspace path, credential-bearing proxy URL, raw ChatGPT payload, unrestricted logs or raw control-plane body.

## 3. Required Cases

### M02-E2E-01 — Start → authoritative Ready → real ChatGPT Tool call

**AC:** M02-AC02, AC03, AC04, AC05, AC15  
**Task:** T6

For the exact candidate package:

1. perform the frozen cold procedure: fully exit the desktop app and candidate-owned Runtime/Tunnel, wait at least three seconds, reopen the same package, then Start;
2. record the Host T0 and the first authoritative Ready T1;
3. after Ready, perform a real ChatGPT Tool call through the accepted external route;
4. prove the tool call reaches Runtime MCP and returns the expected safe result;
5. repeat with the frozen warm procedure after Stop and at least two seconds;
6. collect all counted samples in the external evidence channel.

**PASS:** no false Ready and the real post-Ready Tool call is usable.

### M02-E2E-02 — Packaged one-click diagnostics and shared result

**AC:** M02-AC06, AC07, AC08, AC09, AC13  
**Task:** T5/T6

From packaged Overview:

- run diagnostics;
- verify overall/check status/scope/evidence/remediation/startup detail;
- navigate to an applicable Settings target;
- verify Settings displays the same aggregate result;
- copy only after explicit action and inspect the allowlisted summary.

Repeat after a controlled Stop/reconnect so stale history cannot make current diagnostics PASS.

### M02-E2E-03 — Real fault, stale evidence and recovery

**AC:** M02-AC09, AC10, AC11, AC12  
**Task:** T4/T6

Using only the accepted QA fault procedure for the candidate-owned Tunnel/control-plane path:

- run diagnostics while the fault is live;
- verify FAIL/UNKNOWN and typed remediation;
- verify no automatic Start/Stop/Restart/config/SecretStore mutation occurs;
- restore the external condition;
- verify a new diagnostics run reflects current Ready evidence;
- verify stale old evidence is not reused.

### M02-E2E-04 — Candidate readiness and regression boundary

**AC:** M02-AC05, AC14, AC15  
**Task:** T2/T6

Verify packaged Tray/Close/Stop/Restart/Quit, connection notifications, Workspace/Policy/Approval/Bash/SecretStore behavior and the real post-Ready Tool call remain functional. Intentional Stop/Quit must not be reported as a connection fault.

## 4. E2E Verdict Rules

- PASS — exact package and real boundary executed with all required assertions.
- FAIL — required assertion failed.
- BLOCKED — real external package/network/account/credential boundary unavailable.
- INCONCLUSIVE — evidence cannot distinguish the required boundary.

Local/unit/integration evidence never upgrades a BLOCKED external case to PASS.

## 5. Exit Gate

M02 E2E is PASS only when M02-E2E-01 through M02-E2E-04 are PASS, package identity is stable, all test-owned resources are cleaned up and no raw sensitive payload is retained. E2E PASS does not itself mark M02 ACCEPTED/FROZEN; Manual Acceptance follows.


# M4 Final Acceptance — Human Approval Workflow

> Status: M4 — ACCEPTED / FROZEN
> Manual Acceptance date: 2026-10-02 (Asia/Shanghai)
> Final closeout date: 2026-10-03 (Asia/Shanghai)

## Final decision

M4 — Human Approval Workflow is accepted and frozen. This closeout records existing evidence only; it does not replay the complete QA matrix, change M4 product logic, or start M5.

## Evidence retention

| Gate | Result | Existing evidence reference |
| --- | --- | --- |
| Development | PASS | M4 implementation commit `6ffbabaebec9fbc5b747450b9ff869115b0e25c5` |
| Integration | PASS | `INT-01`–`INT-30`, retained from test/evidence commit `f69de0289e3a0cd43e84893fd02fbe8e437a1948` |
| Local Black-box E2E | PASS | `E2E-01`–`E2E-18`, `LOCAL_M4_BLACK_BOX_E2E=PASS`, retained from `f69de0289e3a0cd43e84893fd02fbe8e437a1948` |
| Manual Acceptance | PASS | Human Reviewer confirmation recorded in `MANUAL_ACCEPTANCE_CHECKLIST.md` |

## Manual Acceptance evidence

The acceptance run used the M4-specific Portable artifact and confirmed:

- External ChatGPT / PDG connected to the current M4 Portable Runtime.
- `read_file` executed successfully.
- Actual Workspace: `C:\AI\HumanContext`.
- Normalized path actually read: `\\?\C:\AI\HumanContext\README.md`.
- The M4 Approval manual workflow passed.

### Non-blocking observation

`glob` produced an internal error during this manual run. It is recorded as a non-blocking observation, is not hidden, and is not reclassified as PASS. Based on the completed M4 Approval evidence, it is not an M4 Approval blocker and does not change the final decision.

## Artifact rule

Manual Acceptance used this dedicated M4 artifact directory:

```text
artifacts/M4-Human-Approval/ChatGPT-Harness_0.1.1_x64-portable/
```

The artifact remains under its own M4 milestone directory and is not shared with or overwritten by another milestone. `/artifacts/` is ignored by `.gitignore` so generated packages remain out of source control while the M4 package remains available locally for the recorded acceptance.

## Verification-flow changes

- `.gitignore` adds `/artifacts/` for milestone build output.
- `ui/scripts/verify-portable.ps1` sends the runtime tool capability configuration used by the M4 Portable verification flow: `readFile`, `glob`, `write`, and `edit` enabled; `bash` disabled. This matches the existing M4 runtime initialization contract.
- No M4 product logic was changed during final closeout.

## Blocker and scope disposition

```text
Open blockers: none
M5: not started
Final status: M4 — ACCEPTED / FROZEN
```

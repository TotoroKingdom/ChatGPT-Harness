# M02 Candidate Comparison — 2026-10-27

状态：`FAIL`（性能阈值未达标；未执行 Integration/E2E；未标记 `ACCEPTED` / `FROZEN`）

## Candidate identity

| Field | Value |
| --- | --- |
| Candidate commit | `3eb7734262b97ebcaf857b02015c0ee203b9e5e8` |
| Candidate parent sequence | B0 `6a2418521fb963934e1486b504df7d7ed06c20ae` → M02 reapplication commit |
| Candidate source dirty | `false` |
| Candidate source state SHA-256 | `FE0D7EDD7E0F456C35A22736236C041C7FC9D4257AAA75C2263B5702BF00B288` |
| Candidate UI SHA-256 | `8D8C90B50AC48F61E19A6038E2ED5BC6C637871AA99D20DE1809C2E7C37D8A2E` |
| Candidate Runtime SHA-256 | `A3C4B040F786737E28FB5B28A4FEC75B107B71294D6B1FE955AD7480545B26DB` |
| Candidate Tunnel SHA-256 | `83F08FB39B1C154747DEBD31B81B65DD4EE834CACF5A073B6301B2688699BC76` |

Candidate retains the verified readiness/diagnostic fixes and does not contain the unsafe 5-second control-plane poll override. No safe material Host-controllable bottleneck was available after B0: the measured wait is dominated by real `control_plane_connected` long-poll time. Therefore no external wait shortcut was introduced.

## Same-environment statistics

| Cohort | B0 Ready / counted | B0 failure rate | B0 P50 | B0 P95 | Candidate Ready / counted | Candidate failure rate | Candidate P50 | Candidate P95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Cold | 30 / 30 | `0` | 31,502 ms | 33,508 ms | 30 / 30 | `0` | 32,572 ms | 66,392 ms |
| Warm | 29 / 30 | `1/30` | 32,079 ms | 66,405 ms | 30 / 30 | `0` | 32,648 ms | 35,210 ms |

## Frozen threshold comparison

| Rule | Frozen requirement | Actual | Verdict |
| --- | --- | --- | --- |
| Cold P50 | Candidate ≤ 28,351.8 ms | 32,572 ms | FAIL |
| Warm P50 | Candidate ≤ 27,267.15 ms | 32,648 ms | FAIL |
| Cold P95 guard | Candidate ≤ 35,183.4 ms | 66,392 ms | FAIL |
| Warm P95 guard | Candidate ≤ 69,725.25 ms | 35,210 ms | PASS |
| At least one P95 ≤ 90% B0 | Cold ≤ 30,157.2 or warm ≤ 59,764.5 ms | Warm passes this individual tail rule | PASS |
| Failure rate | Candidate ≤ B0 and ≤ 1/30; B0 zero implies candidate zero | Cold 0 ≤ 0; warm 0 ≤ 1/30 | PASS |
| Ready sample gate | At least 29 each cohort | Cold 30, warm 30 | PASS |
| Material bottleneck A/B | One frozen clause must pass | No qualifying Host phase improvement; both total P95 do not improve 10% | FAIL |

Overall performance verdict: `FAIL`. No samples were removed for being slow, and no threshold was changed.

## Raw candidate evidence

- [Candidate cold aggregate](C:/AI/ChatGPT-Harness/docs-codex/v4/evidence/M02-startup-performance-diagnostics/raw-Candidate-20261027/Candidate-cold-aggregate.ndjson) — 32 lines, SHA-256 `00097C532E8D8F411CCCB0B8E6367F11E60CAFECAC237CAEBC78557795B424C9`
- [Candidate warm aggregate](C:/AI/ChatGPT-Harness/docs-codex/v4/evidence/M02-startup-performance-diagnostics/raw-Candidate-20261027/Candidate-warm-aggregate.ndjson) — 33 lines, SHA-256 `D5012669C41DE949C86908C2D6EF318B837FF95FF1FB3843DD8B981D63B56378`

The candidate is not eligible to enter INT-01～16 or E2E-01～04 because the frozen performance gate failed. This is an honest performance failure, not an environment/authorization BLOCKED verdict and not an acceptance decision.

import { expect, it } from "vitest";
import { editorFromRule, ruleFromEditor } from "./RuleEditor";
import type { UserPolicyRuleConfig } from "@protocol/v1";

it("does not normalize existing user metadata when another rule field changes", () => {
  const source: UserPolicyRuleConfig = { id: "existing-ID", name: " My original name ", reason: " My original reason ", decision: "ask", enabled: true, matcher: { type: "bash_command", executable: "GIT.EXE", argvPrefix: ["status", "", "two words"] } };
  const result = ruleFromEditor({ ...editorFromRule(source), decision: "deny" });
  expect(result.rule).toEqual({ ...source, decision: "deny" });
});

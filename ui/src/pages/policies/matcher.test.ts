import { expect, it } from "vitest";
import { naturalCommandText, normalizeExecutable, parseLiteralMatcher, parseNaturalCommand } from "./matcher";

it("creates normalized executable and logical token prefixes, including appended argument scope", () => {
  expect(parseNaturalCommand("GIT.EXE status").matcher).toEqual({ type: "bash_command", executable: "git", argvPrefix: ["status"] });
  expect(parseNaturalCommand("git status --short").matcher).toEqual({ type: "bash_command", executable: "git", argvPrefix: ["status", "--short"] });
  expect(parseNaturalCommand("git statusx").matcher).not.toEqual(parseNaturalCommand("git status").matcher);
  expect(parseNaturalCommand("git").matcher).toEqual({ type: "bash_command", executable: "git", argvPrefix: [] });
});
it.each(["", "git 'status'", 'git "status"', "git sta\\tus", "git $ARGS", "git *", "git status > out", "git status | cat", "git status && pwd", "git status; pwd", "git $(pwd)", "git `pwd`", "git status\npwd", "X=1 git status", "git status\0"])("rejects unsupported natural syntax: %j", (text) => {
  expect(parseNaturalCommand(text).matcher).toBeUndefined();
  expect(parseNaturalCommand(text).error).toBeTruthy();
});
it("honors the existing 64-prefix-token and 1024-UTF8-byte bounds", () => {
  expect(parseNaturalCommand("git " + Array(64).fill("a").join(" ")).error).toBeUndefined();
  expect(parseNaturalCommand("git " + Array(65).fill("a").join(" ")).error).toBe("validation.policies.argvPrefixMaxTokens");
  expect(parseLiteralMatcher("git", JSON.stringify(["é".repeat(512)])).error).toBeUndefined();
  expect(parseLiteralMatcher("git", JSON.stringify(["é".repeat(513)])).error).toBe("validation.policies.argvPrefixTokenBytes");
  expect(normalizeExecutable("a".repeat(1025))).toBeNull();
});
it("preserves empty and whitespace literal argv tokens without shell decoding", () => {
  const matcher = { type: "bash_command" as const, executable: "git", argvPrefix: ["", "two words", "$literal", '"quote"'] };
  expect(parseLiteralMatcher("git", JSON.stringify(matcher.argvPrefix)).matcher).toEqual(matcher);
  expect(naturalCommandText(matcher)).toBeNull();
  expect(parseLiteralMatcher("git", '["a\\u0000b"]').error).toBe("validation.policies.argvPrefixNul");
});
it("uses only ASCII case normalization like the frozen protocol", () => {
  expect(normalizeExecutable("C:\\TOOLS\\Git.EXE")).toBe("c:/tools/git");
  expect(normalizeExecutable("ÄBC.exe")).toBe("Äbc");
  expect(normalizeExecutable("git\u0001")).toBeNull();
  expect(naturalCommandText({ type: "bash_command", executable: "git", argvPrefix: ["status"] })).toBe("git status");
});

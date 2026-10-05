import type { PolicyMatcherConfig } from "@protocol/v1";

export type MatcherResult = { matcher?: PolicyMatcherConfig; error?: string };
const literal = /^[\p{L}\p{N}_\-./:@+%,]+$/u;
const bytes = (value: string) => new TextEncoder().encode(value).byteLength;

/** Mirrors only the persisted matcher input contract, never Runtime evaluation. */
export function normalizeExecutable(value: string): string | null {
  const text = value.trim();
  if (!text || bytes(text) > 1024 || /[\s\p{Cc}"';&|<>$`(){}\[\]?*~#=]/u.test(text)) return null;
  const normalized = text.replace(/\\/g, "/").replace(/[A-Z]/g, (letter) => letter.toLowerCase()).replace(/\.exe$/, "");
  return normalized || null;
}

function withPrefix(executable: string, argvPrefix: string[]): MatcherResult {
  const normalized = normalizeExecutable(executable);
  if (!normalized) return { error: "validation.policies.executableInvalid" };
  if (argvPrefix.length > 64) return { error: "validation.policies.argvPrefixMaxTokens" };
  if (argvPrefix.some((token) => token.includes("\0"))) return { error: "validation.policies.argvPrefixNul" };
  if (argvPrefix.some((token) => bytes(token) > 1024)) return { error: "validation.policies.argvPrefixTokenBytes" };
  return { matcher: { type: "bash_command", executable: normalized, argvPrefix } };
}

/** A deterministic literal-only single command; unsupported syntax never broadens scope. */
export function parseNaturalCommand(text: string): MatcherResult {
  if (/[\r\n]/.test(text)) return { error: "m8Policies.naturalUnsupported" };
  const tokens = text.trim().split(/[ \t]+/);
  if (!tokens.every((token) => literal.test(token))) return { error: "m8Policies.naturalUnsupported" };
  return withPrefix(tokens[0], tokens.slice(1));
}

export function parseLiteralMatcher(executable: string, argvJsonText: string): MatcherResult {
  let prefix: unknown;
  try { prefix = JSON.parse(argvJsonText); }
  catch { return { error: "validation.policies.argvPrefixJson" }; }
  if (!Array.isArray(prefix) || !prefix.every((token): token is string => typeof token === "string")) {
    return { error: "validation.policies.argvPrefixStrings" };
  }
  return withPrefix(executable, prefix);
}

/** null means use the lossless Advanced editor rather than reinterpret existing values. */
export function naturalCommandText(matcher: PolicyMatcherConfig): string | null {
  if (matcher.type !== "bash_command") return null;
  const tokens = [matcher.executable, ...matcher.argvPrefix];
  if (!tokens.every((token) => literal.test(token)) || normalizeExecutable(matcher.executable) !== matcher.executable) return null;
  const text = tokens.join(" ");
  const converted = parseNaturalCommand(text).matcher;
  return JSON.stringify(converted) === JSON.stringify(matcher) ? text : null;
}

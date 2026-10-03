import { describe, expect, it } from "vitest";
import en from "./en.json";
import zhCN from "./zh-CN.json";
import { i18n } from "./index";
import { supportedLanguages } from "./resolve-language";

function flatten(value: Record<string, unknown>, prefix = ""): Record<string, string> {
  const entries: Record<string, string> = {};
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof child === "string") entries[path] = child;
    else {
      expect(child, path).toBeTypeOf("object");
      Object.assign(entries, flatten(child as Record<string, unknown>, path));
    }
  }
  return entries;
}
const placeholders = (value: string) => [...value.matchAll(/\{\{\s*([^},\s]+)(?:,[^}]*)?\s*\}\}/g)].map((match) => match[1]).sort();

describe("supported language resource integrity", () => {
  it("has exact nonempty key and placeholder parity without relying on fallback", () => {
    const english = flatten(en), chinese = flatten(zhCN);
    expect(Object.keys(chinese).sort()).toEqual(Object.keys(english).sort());
    for (const [key, value] of Object.entries(english)) {
      expect(value.trim(), key).not.toBe("");
      expect(chinese[key].trim(), key).not.toBe("");
      expect(placeholders(chinese[key]), key).toEqual(placeholders(value));
    }
  });
  it("covers every primary domain with exactly two supported languages", () => {
    expect(supportedLanguages).toEqual(["en", "zh-CN"]);
    expect(i18n.options.fallbackLng).toEqual(["en"]);
    for (const domain of ["app", "common", "nav", "status", "overview", "tools", "policies", "activity", "approvals", "settings", "firstRun", "errors", "policy", "bash", "validation"]) {
      expect(Object.keys(en), domain).toContain(domain);
      expect(Object.keys(zhCN), domain).toContain(domain);
    }
  });
});

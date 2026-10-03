import fs from "node:fs";
import path from "node:path";
import ts from "typescript";
import { expect, it } from "vitest";
import en from "./en.json";
import zhCN from "./zh-CN.json";

// Bounded presentation audit using the existing TypeScript parser. No runtime,
// protocol, user content, CSS or test fixture literals are treated as UI copy.
const root = path.resolve("src");
const dirs = ["pages", "components"];
const allowedIdentity = new Set(["H", "Harness", "ChatGPT Harness", "MCP", "Runtime", "Tunnel"]);
const allowedExamples = new Set(["http://127.0.0.1:7897", "git", '["status", "--short"]']);
const visibleAttributes = new Set(["title", "label", "aria-label", "placeholder", "description", "eyebrow"]);
const keyPattern = /^(?:app|common|nav|status|enum|overview|tools|policies|activity|approvals|settings|firstRun|errors|policy|bash|validation)\./;
function files(directory: string): string[] {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? files(file) : /\.tsx?$/.test(file) && !file.includes(".test.") ? [file] : [];
  });
}
function exists(resources: object, key: string): boolean {
  let value: unknown = resources;
  for (const part of key.split(".")) {
    if (value === null || typeof value !== "object") return false;
    value = (value as Record<string, unknown>)[part];
  }
  return typeof value === "string" && value.trim().length > 0;
}

it("has no hard-coded primary JSX copy or untranslated visible attributes", () => {
  const issues: string[] = [];
  for (const file of dirs.flatMap((dir) => files(path.join(root, dir)))) {
    const source = ts.createSourceFile(file, fs.readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    function visit(node: ts.Node) {
      const line = source.getLineAndCharacterOfPosition(node.getStart()).line + 1;
      const report = (text: string) => issues.push(`${path.relative(root, file)}:${line}: ${text}`);
      if (ts.isJsxText(node)) {
        const text = node.getText().replace(/\s+/g, " ").trim();
        if (/[a-zA-Z]/.test(text) && !allowedIdentity.has(text)) report(text);
      }
      if (ts.isJsxAttribute(node) && visibleAttributes.has(node.name.getText()) && node.initializer && ts.isStringLiteral(node.initializer)) {
        const text = node.initializer.text;
        if (text && !allowedExamples.has(text) && !allowedIdentity.has(text)) report(text);
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
  expect(issues).toEqual([]);
});

it("resolves every literal presentation key directly in both locales", () => {
  const issues: string[] = [];
  const sources = [...dirs.flatMap((dir) => files(path.join(root, dir))), path.join(root, "i18n/presentation.ts"), path.join(root, "app/router/routes.ts")];
  for (const file of sources) {
    const source = ts.createSourceFile(file, fs.readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    function visit(node: ts.Node) {
      if (ts.isStringLiteral(node) && keyPattern.test(node.text)) {
        for (const [locale, resources] of [["en", en], ["zh-CN", zhCN]] as const) {
          if (!exists(resources, node.text)) issues.push(`${path.relative(root, file)}: ${locale}: ${node.text}`);
        }
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
  expect(issues).toEqual([]);
});

// M7 black-box probe: evaluate the production presentation code on real Control DTOs.
// Input comes from the release-Runtime E2E runner; no Runtime or mapper is mocked.
import { build } from "esbuild";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const source = fileURLToPath(new URL("../src/i18n/presentation.ts", import.meta.url));
const compiled = await build({ entryPoints: [source], bundle: true, write: false, format: "esm", platform: "node", logLevel: "silent" });
const production = await import(`data:text/javascript;base64,${Buffer.from(compiled.outputFiles[0].text).toString("base64")}`);
const locales = await Promise.all(["en", "zh-CN"].map(async (language) => [language, JSON.parse(await readFile(new URL(`../src/i18n/${language}.json`, import.meta.url), "utf8"))]));
let input = "";
for await (const chunk of process.stdin) input += chunk;
const { cases } = JSON.parse(input);
assert.ok(Array.isArray(cases) && cases.length >= 4);
const result = cases.map(({ label, dto, tool, expectedReason, expectedRemediationKey, expectedAvailability }) => {
  assert.ok(Array.isArray(dto), "expected the real effective-tools array");
  const actual = dto.find((entry) => entry.name === tool);
  assert.ok(actual, "tool missing from real Control DTO");
  assert.equal(actual.statusReason, expectedReason);
  assert.equal(actual.availability, expectedAvailability);
  const remediationKey = production.toolRemediationKey(actual.statusReason);
  assert.equal(remediationKey, expectedRemediationKey);
  const reasonKey = production.toolStatusReasonKey(actual.statusReason);
  const presentation = Object.fromEntries(locales.map(([language, resource]) => {
    const lookup = (key) => key.split(".").reduce((value, part) => value?.[part], resource);
    const reason = lookup(reasonKey);
    const remediation = lookup(remediationKey);
    assert.equal(typeof reason, "string");
    assert.equal(typeof remediation, "string");
    assert.ok(reason.length && remediation.length);
    return [language, { reason, remediation }];
  }));
  return { label, tool, statusReason: actual.statusReason, availability: actual.availability, remediationKey, reasonKey, presentation };
});
process.stdout.write(JSON.stringify({ productionSource: "ui/src/i18n/presentation.ts", cases: result }));

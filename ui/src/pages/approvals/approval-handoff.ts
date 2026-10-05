import type { ApprovalRequest, PolicyMatcherConfig } from "@protocol/v1";
import { normalizeExecutable, parseLiteralMatcher } from "@/pages/policies/matcher";

/** This creates only a configuration draft, never a decision or session grant. */
export function safeApprovalMatcher(request: ApprovalRequest): PolicyMatcherConfig | undefined {
  if (["read_file", "glob", "write", "edit"].includes(request.tool)) return { type: "tool", tool: request.tool };
  if (request.tool !== "bash") return undefined;
  const details = request.bashDetails;
  if (!details || details.classification !== "classified" || details.previewRedacted !== false || details.previewTruncated !== false || !Array.isArray(details.normalizedSegments) || details.normalizedSegments.length !== 1) return undefined;
  const segment = details.normalizedSegments[0];
  if (!segment || segment.argvRedacted !== false || typeof segment.executable !== "string" || !Array.isArray(segment.argv) || !segment.argv.every((token) => typeof token === "string") || normalizeExecutable(segment.executable) !== segment.executable) return undefined;
  // Frozen matcher validation bounds; no shell parsing of the display preview.
  return parseLiteralMatcher(segment.executable, JSON.stringify(segment.argv)).matcher;
}

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";

const FROZEN_PROTOCOL = "2025-03-26";
const endpoint = process.argv[2];

if (!endpoint) {
  console.error("Usage: node client.mjs <runtime-mcp-url>");
  process.exit(2);
}

class FrozenProtocolTransport {
  constructor(url) {
    this.inner = new StreamableHTTPClientTransport(new URL(url));
  }

  get sessionId() {
    return this.inner.sessionId;
  }

  set onclose(handler) {
    this.inner.onclose = handler;
  }

  set onerror(handler) {
    this.inner.onerror = handler;
  }

  set onmessage(handler) {
    this.inner.onmessage = handler;
  }

  async start() {
    return this.inner.start();
  }

  async close() {
    return this.inner.close();
  }

  setProtocolVersion(version) {
    return this.inner.setProtocolVersion(version);
  }

  async send(message, options) {
    const frozen = Array.isArray(message)
      ? message.map((item) => this.freezeInitialize(item))
      : this.freezeInitialize(message);
    return this.inner.send(frozen, options);
  }

  freezeInitialize(message) {
    if (message?.method !== "initialize") {
      return message;
    }
    return {
      ...message,
      params: {
        ...message.params,
        protocolVersion: FROZEN_PROTOCOL,
      },
    };
  }
}

function firstText(result) {
  return result?.content?.find((item) => item?.type === "text")?.text ?? null;
}

function parseCanonicalText(result) {
  const text = firstText(result);
  if (!text) {
    throw new Error("SDK tool result omitted canonical TextContent");
  }
  return JSON.parse(text);
}

function parseToolObservation(result) {
  return {
    isError: result?.isError === true,
    payload: parseCanonicalText(result),
    wire: JSON.stringify(result),
  };
}

const transport = new FrozenProtocolTransport(endpoint);
const client = new Client(
  { name: "chatgpt-harness-m10-sdk-client", version: "1.0.0" },
  { capabilities: {} },
);

try {
  await client.connect(transport);
  const listed = await client.listTools();
  const toolNames = listed.tools.map((tool) => tool.name);
  const readResult = await client.callTool({
    name: "read_file",
    arguments: { path: "m10-safe-read.txt" },
  });
  const readObservation = parseToolObservation(readResult);

  const safeErrorResult = await client.callTool({
    name: "read_file",
    arguments: { path: "../outside-m10-secret.txt" },
  });
  const safeError = parseToolObservation(safeErrorResult);
  if (!safeError.isError || safeError.payload.code !== "WORKSPACE_VIOLATION") {
    throw new Error("SDK safe-error call did not return WORKSPACE_VIOLATION as a tool error");
  }
  if (safeError.wire.includes("outside-m10-secret.txt")) {
    throw new Error("SDK safe-error wire leaked the requested physical/logical path");
  }

  console.log(
    JSON.stringify({
      status: "PASS",
      client: "@modelcontextprotocol/sdk",
      clientVersion: "1.29.0",
      protocolRevision: FROZEN_PROTOCOL,
      endpoint,
      toolNames,
      readPayload: readObservation.payload,
      readIsError: readObservation.isError,
      wireHasStructuredContent: readObservation.wire.includes("structuredContent"),
      safeError,
    }),
  );
} finally {
  await client.close();
}

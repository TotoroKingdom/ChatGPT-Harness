import { expect, it } from "vitest";
import { toClientError } from "./errors";

it("preserves machine errors with or without fallback prose", () => {
  expect(toClientError({ code: "CONFIG_WRITE_FAILED" }).code).toBe("CONFIG_WRITE_FAILED");
  expect(toClientError(Object.assign(new Error("diagnostic"), { code: "BASH_TIMEOUT" })).code).toBe("BASH_TIMEOUT");
  expect(toClientError({ code: "CONFIG_INVALID", message: "diagnostic" }).message).toBe("diagnostic");
  expect(toClientError(null).message).toBe("");
});

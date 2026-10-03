export type ClientErrorKind = "disconnected" | "error";

export class HarnessClientError extends Error {
  readonly kind: ClientErrorKind;
  readonly cause?: unknown;
  readonly code?: string;

  constructor(kind: ClientErrorKind, message: string, cause?: unknown, code?: string) {
    super(message);
    this.name = "HarnessClientError";
    this.kind = kind;
    this.cause = cause;
    this.code = code;
  }
}

export function toClientError(error: unknown): HarnessClientError {
  if (error instanceof HarnessClientError) return error;
  if (error instanceof Error) {
    const code = "code" in error && typeof error.code === "string" ? error.code : undefined;
    return new HarnessClientError("error", error.message, error, code);
  }
  if (typeof error === "object" && error !== null) {
    const value = error as { code?: unknown; message?: unknown };
    if (typeof value.message === "string" || typeof value.code === "string") {
      return new HarnessClientError("error", typeof value.message === "string" ? value.message : "", error, typeof value.code === "string" ? value.code : undefined);
    }
  }
  return new HarnessClientError("error", "", error);
}

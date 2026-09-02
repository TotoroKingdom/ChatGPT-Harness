export type ClientErrorKind = "disconnected" | "error";

export class HarnessClientError extends Error {
  readonly kind: ClientErrorKind;
  readonly cause?: unknown;

  constructor(kind: ClientErrorKind, message: string, cause?: unknown) {
    super(message);
    this.name = "HarnessClientError";
    this.kind = kind;
    this.cause = cause;
  }
}

export function toClientError(error: unknown): HarnessClientError {
  if (error instanceof HarnessClientError) return error;
  if (error instanceof Error) {
    return new HarnessClientError("error", error.message, error);
  }
  return new HarnessClientError("error", "The Harness client returned an unknown error.", error);
}

import { ErrorCode, ErrorDomain, ProtocolErrorSchema, create } from "@ym-connect/protocol";

export class YmConnectError extends Error {
  constructor(message, options = {}) {
    super(message, { cause: options.cause });
    this.name = new.target.name;
    this.domain = options.domain ?? ErrorDomain.ERROR_DOMAIN_INTERNAL;
    this.code = options.code ?? ErrorCode.ERROR_CODE_INTERNAL_FAILURE;
    this.retryable = options.retryable ?? false;
    this.retryAfterMs = normalizeRetryAfter(options.retryAfterMs);
    this.metadata = normalizeMetadata(options.metadata);
  }
}

export class ValidationError extends YmConnectError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      domain: ErrorDomain.ERROR_DOMAIN_VALIDATION,
      code: options.code ?? ErrorCode.ERROR_CODE_MALFORMED_MESSAGE,
    });
  }
}

export class CompatibilityError extends YmConnectError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      domain: ErrorDomain.ERROR_DOMAIN_PROTOCOL,
      code: options.code ?? ErrorCode.ERROR_CODE_UNSUPPORTED_PROTOCOL,
    });
  }
}

export class CapabilityError extends YmConnectError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      domain: ErrorDomain.ERROR_DOMAIN_PROTOCOL,
      code: options.code ?? ErrorCode.ERROR_CODE_INCOMPATIBLE_CAPABILITIES,
    });
  }
}

export class FramingError extends YmConnectError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      domain: ErrorDomain.ERROR_DOMAIN_TRANSPORT,
      code: options.code ?? ErrorCode.ERROR_CODE_MALFORMED_MESSAGE,
    });
  }
}

export function toProtocolError(error) {
  if (error instanceof YmConnectError) {
    return create(ProtocolErrorSchema, {
      domain: error.domain,
      code: error.code,
      message: error.message,
      retryable: error.retryable,
      retryAfterMs: BigInt(error.retryAfterMs),
      metadata: error.metadata,
    });
  }
  if (isProtocolError(error)) {
    return create(ProtocolErrorSchema, {
      domain: error.domain,
      code: error.code,
      message: error.message,
      retryable: error.retryable ?? false,
      retryAfterMs: error.retryAfterMs ?? 0n,
      metadata: error.metadata ?? {},
    });
  }
  const message = error instanceof Error ? error.message : String(error);
  return create(ProtocolErrorSchema, {
    domain: ErrorDomain.ERROR_DOMAIN_INTERNAL,
    code: ErrorCode.ERROR_CODE_INTERNAL_FAILURE,
    message,
    retryable: false,
  });
}

export function fromProtocolError(protocolError, options = {}) {
  if (!isProtocolError(protocolError)) {
    throw new TypeError("protocolError must be a ProtocolError-shaped object");
  }
  return new YmConnectError(protocolError.message || "YM Connect protocol error", {
    ...options,
    domain: protocolError.domain,
    code: protocolError.code,
    retryable: protocolError.retryable,
    retryAfterMs: bigintToSafeNumber(protocolError.retryAfterMs ?? 0n),
    metadata: protocolError.metadata,
  });
}

export function isProtocolError(value) {
  return (
    typeof value === "object" &&
    value !== null &&
    Number.isInteger(value.domain) &&
    Number.isInteger(value.code) &&
    typeof value.message === "string"
  );
}

function normalizeMetadata(value) {
  if (value === undefined) return Object.freeze({});
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("metadata must be an object with string values");
  }
  const metadata = {};
  for (const [key, item] of Object.entries(value)) {
    if (typeof item !== "string") throw new TypeError(`metadata.${key} must be a string`);
    metadata[key] = item;
  }
  return Object.freeze(metadata);
}

function normalizeRetryAfter(value) {
  if (value === undefined) return 0;
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError("retryAfterMs must be a non-negative safe integer");
  }
  return value;
}

function bigintToSafeNumber(value) {
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= 0 ? number : Number.MAX_SAFE_INTEGER;
}

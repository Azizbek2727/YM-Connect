/** Canonical repository release version. */
export const REPOSITORY_VERSION = "0.1.0";

/** Current wire protocol emitted by this repository. */
export const PROTOCOL_VERSION = Object.freeze({ major: 1, minor: 0, patch: 0 });

/** Oldest protocol accepted by this repository. */
export const MIN_SUPPORTED_PROTOCOL_VERSION = Object.freeze({ major: 1, minor: 0, patch: 0 });

/** Newest protocol accepted by this repository. */
export const MAX_SUPPORTED_PROTOCOL_VERSION = Object.freeze({ major: 1, minor: 3, patch: 0 });

/** Protocol compatibility window advertised by default. */
export const SUPPORTED_PROTOCOL_RANGE = Object.freeze({
  minimum: MIN_SUPPORTED_PROTOCOL_VERSION,
  maximum: MAX_SUPPORTED_PROTOCOL_VERSION,
});

const VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/u;

export function formatProtocolVersion(version) {
  const normalized = normalizeProtocolVersion(version);
  return `${normalized.major}.${normalized.minor}.${normalized.patch}`;
}

export function parseProtocolVersion(value) {
  if (typeof value !== "string") {
    throw new TypeError("protocol version must be a string");
  }
  const match = VERSION_PATTERN.exec(value);
  if (!match) {
    throw new RangeError(`invalid protocol version: ${value}`);
  }
  return Object.freeze({
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  });
}

export function normalizeProtocolVersion(version) {
  if (typeof version !== "object" || version === null || Array.isArray(version)) {
    throw new TypeError("protocol version must be an object");
  }
  const major = normalizeComponent(version.major, "major");
  const minor = normalizeComponent(version.minor, "minor");
  const patch = normalizeComponent(version.patch, "patch");
  return Object.freeze({ major, minor, patch });
}

function normalizeComponent(value, name) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new RangeError(`${name} must be an unsigned 32-bit integer`);
  }
  return value;
}

const PREFIX_PATTERN = /^[a-z][a-z0-9-]{0,31}$/u;
const OPAQUE_ID_PATTERN = /^[a-z][a-z0-9-]{0,31}_[A-Za-z0-9_-]{22,86}$/u;

export function generateOpaqueId(prefix, byteLength = 16, cryptoProvider = globalThis.crypto) {
  if (!PREFIX_PATTERN.test(prefix)) {
    throw new RangeError("identifier prefix must start with a lowercase letter and contain only lowercase letters, digits, or hyphens");
  }
  if (!Number.isInteger(byteLength) || byteLength < 16 || byteLength > 64) {
    throw new RangeError("identifier entropy length must be between 16 and 64 bytes");
  }
  if (!cryptoProvider || typeof cryptoProvider.getRandomValues !== "function") {
    throw new Error("a cryptographically secure getRandomValues implementation is required");
  }
  const bytes = new Uint8Array(byteLength);
  cryptoProvider.getRandomValues(bytes);
  return `${prefix}_${base64UrlEncode(bytes)}`;
}

export function generateMessageId(cryptoProvider) {
  return generateOpaqueId("msg", 16, cryptoProvider);
}

export function generateCommandId(cryptoProvider) {
  return generateOpaqueId("cmd", 16, cryptoProvider);
}

export function generateSessionId(cryptoProvider) {
  return generateOpaqueId("session", 24, cryptoProvider);
}

export function generateInstanceId(cryptoProvider) {
  return generateOpaqueId("instance", 16, cryptoProvider);
}

export function isOpaqueId(value, expectedPrefix) {
  if (typeof value !== "string" || !OPAQUE_ID_PATTERN.test(value)) return false;
  return expectedPrefix === undefined || value.startsWith(`${expectedPrefix}_`);
}

export function assertOpaqueId(value, expectedPrefix) {
  if (!isOpaqueId(value, expectedPrefix)) {
    throw new RangeError(
      expectedPrefix === undefined
        ? "value is not a valid opaque identifier"
        : `value is not a valid ${expectedPrefix} identifier`,
    );
  }
  return value;
}

function base64UrlEncode(bytes) {
  if (typeof Buffer !== "undefined") {
    return Buffer.from(bytes).toString("base64url");
  }
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}

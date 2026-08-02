import {
  deserializeBinary,
  deserializeJson,
  serializeBinary,
  serializeJson,
} from "./serialization.js";

export const systemClock = Object.freeze({
  nowUnixMs() {
    return BigInt(Date.now());
  },
});

export function createCodec(schema) {
  if (!schema || typeof schema !== "object") throw new TypeError("schema is required");
  return Object.freeze({
    encode(message) {
      return serializeBinary(schema, message);
    },
    decode(bytes, options) {
      return deserializeBinary(schema, bytes, options);
    },
    toJson(message, options) {
      return serializeJson(schema, message, options);
    },
    fromJson(json) {
      return deserializeJson(schema, json);
    },
  });
}

export function createValidator(validate) {
  if (typeof validate !== "function") throw new TypeError("validate must be a function");
  return Object.freeze({ validate });
}

export function createCompatibilityPolicy(negotiate) {
  if (typeof negotiate !== "function") throw new TypeError("negotiate must be a function");
  return Object.freeze({ negotiate });
}

export function createCapabilityProvider(getCapabilities) {
  if (typeof getCapabilities !== "function") {
    throw new TypeError("getCapabilities must be a function");
  }
  return Object.freeze({ getCapabilities });
}

export function assertClock(value) {
  if (!value || typeof value.nowUnixMs !== "function") {
    throw new TypeError("clock must expose nowUnixMs()");
  }
  const now = value.nowUnixMs();
  if (typeof now !== "bigint" || now < 0n) {
    throw new TypeError("clock nowUnixMs() must return a non-negative bigint");
  }
  return value;
}

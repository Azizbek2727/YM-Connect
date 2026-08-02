import {
  Capability,
  CapabilityNegotiationSchema,
  CapabilitySetSchema,
  create,
} from "@ym-connect/protocol";
import { CapabilityError } from "./errors.js";

const KNOWN_CAPABILITY_VALUES = new Set(Object.values(Capability));
const CAPABILITY_NAMES = new Map(Object.entries(Capability).map(([name, value]) => [value, name]));

export const DEFAULT_BASE_CAPABILITIES = Object.freeze([
  Capability.CAPABILITY_PLAYBACK_READ,
  Capability.CAPABILITY_PLAY,
  Capability.CAPABILITY_PAUSE,
  Capability.CAPABILITY_TOGGLE_PLAY_PAUSE,
  Capability.CAPABILITY_NEXT,
  Capability.CAPABILITY_PREVIOUS,
  Capability.CAPABILITY_TIMELINE_UPDATES,
  Capability.CAPABILITY_ENCRYPTED_TRANSPORT,
  Capability.CAPABILITY_REPLAY_PROTECTION,
]);

export function capabilityName(capability) {
  return CAPABILITY_NAMES.get(capability) ?? `CAPABILITY_UNKNOWN_${capability}`;
}

export function normalizeCapabilityList(values, options = {}) {
  if (!Array.isArray(values)) {
    throw new TypeError("capability list must be an array");
  }
  const allowUnknown = options.allowUnknown ?? false;
  const normalized = [];
  for (const value of values) {
    if (!Number.isInteger(value) || value < 0 || value > 0x7fff_ffff) {
      throw new RangeError(`invalid capability value: ${String(value)}`);
    }
    if (value === Capability.CAPABILITY_UNSPECIFIED) continue;
    if (!allowUnknown && !KNOWN_CAPABILITY_VALUES.has(value)) {
      throw new CapabilityError(`unknown capability value: ${value}`);
    }
    normalized.push(value);
  }
  return Object.freeze([...new Set(normalized)].sort((left, right) => left - right));
}

export function normalizeCapabilitySet(value, options = {}) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("capability set must be an object");
  }
  const supported = normalizeCapabilityList(value.supported ?? [], options);
  const required = normalizeCapabilityList(value.required ?? [], options);
  const supportedSet = new Set(supported);
  const missingLocally = required.filter((capability) => !supportedSet.has(capability));
  if (missingLocally.length > 0) {
    throw new CapabilityError(
      `required capabilities are not included in supported: ${missingLocally.map(capabilityName).join(", ")}`,
      { metadata: { missing: missingLocally.map(String).join(",") } },
    );
  }
  const parameters = normalizeParameters(value.parameters ?? {});
  return create(CapabilitySetSchema, { supported, required, parameters });
}

export function hasCapability(capabilitySet, capability) {
  return normalizeCapabilitySet(capabilitySet, { allowUnknown: true }).supported.includes(
    capability,
  );
}

export function requireCapabilities(capabilitySet, requiredCapabilities) {
  const normalized = normalizeCapabilitySet(capabilitySet, { allowUnknown: true });
  const required = normalizeCapabilityList(requiredCapabilities, { allowUnknown: true });
  const supported = new Set(normalized.supported);
  const missing = required.filter((capability) => !supported.has(capability));
  if (missing.length > 0) {
    throw new CapabilityError(
      `missing required capabilities: ${missing.map(capabilityName).join(", ")}`,
      { metadata: { missing: missing.map(String).join(",") } },
    );
  }
  return normalized;
}

export function negotiateCapabilities(localValue, remoteValue, options = {}) {
  const local = normalizeCapabilitySet(localValue, options);
  const remote = normalizeCapabilitySet(remoteValue, options);
  const localSupported = new Set(local.supported);
  const remoteSupported = new Set(remote.supported);
  const negotiatedSupported = local.supported.filter((capability) =>
    remoteSupported.has(capability),
  );
  const required = normalizeCapabilityList([...local.required, ...remote.required], options);
  const missingRequired = required.filter(
    (capability) => !localSupported.has(capability) || !remoteSupported.has(capability),
  );
  const negotiatedRequired = required.filter((capability) => !missingRequired.includes(capability));
  const parameters = {};
  for (const key of Object.keys(local.parameters).sort()) {
    if (Object.hasOwn(remote.parameters, key) && remote.parameters[key] === local.parameters[key]) {
      parameters[key] = local.parameters[key];
    }
  }
  return create(CapabilityNegotiationSchema, {
    offered: local,
    available: remote,
    negotiated: create(CapabilitySetSchema, {
      supported: negotiatedSupported,
      required: negotiatedRequired,
      parameters,
    }),
    missingRequired,
  });
}

function normalizeParameters(parameters) {
  if (typeof parameters !== "object" || parameters === null || Array.isArray(parameters)) {
    throw new TypeError("capability parameters must be an object");
  }
  const normalized = {};
  for (const key of Object.keys(parameters).sort()) {
    if (key.length === 0 || key.length > 128) {
      throw new RangeError("capability parameter names must contain 1 to 128 characters");
    }
    const value = parameters[key];
    if (typeof value !== "string" || value.length > 1024) {
      throw new RangeError(
        `capability parameter ${key} must be a string of at most 1024 characters`,
      );
    }
    normalized[key] = value;
  }
  return Object.freeze(normalized);
}

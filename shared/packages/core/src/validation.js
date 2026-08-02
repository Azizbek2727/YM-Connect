import {
  ClientEnvelopeSchema,
  ConnectorEnvelopeSchema,
  ErrorCode,
  ErrorDomain,
  PlaybackStatus,
  PlayerHealth,
  RepeatMode,
  toBinary,
} from "@ym-connect/protocol";
import { normalizeCapabilitySet } from "./capabilities.js";
import { ValidationError } from "./errors.js";
import { normalizeProtocolVersion } from "./version.js";

export const VALIDATION_LIMITS = Object.freeze({
  identifier: 256,
  displayName: 256,
  provider: 128,
  title: 1024,
  text: 4096,
  url: 4096,
  metadataEntries: 64,
  binaryField: 1_048_576,
  envelope: 1_048_576,
});

const IDENTIFIER_PATTERN = /^[A-Za-z0-9._:@/-]+$/u;
const COMMAND_CASES = new Set([
  "play",
  "pause",
  "stop",
  "togglePlayPause",
  "seekAbsolute",
  "seekRelative",
  "next",
  "previous",
  "setVolume",
  "setMuted",
  "setShuffle",
  "setRepeat",
  "setLike",
]);
const ENUMS = Object.freeze({
  errorCode: new Set(Object.values(ErrorCode)),
  errorDomain: new Set(Object.values(ErrorDomain)),
  playbackStatus: new Set(Object.values(PlaybackStatus)),
  playerHealth: new Set(Object.values(PlayerHealth)),
  repeatMode: new Set(Object.values(RepeatMode)),
});

export function validateIdentifier(value, fieldName = "identifier", options = {}) {
  const allowEmpty = options.allowEmpty ?? false;
  assertString(value, fieldName, allowEmpty ? 0 : 1, options.maxLength ?? VALIDATION_LIMITS.identifier);
  if (value.length > 0 && !IDENTIFIER_PATTERN.test(value)) {
    fail(fieldName, "contains unsupported characters");
  }
  return value;
}

export function validateProtocolVersion(value) {
  return normalizeProtocolVersion(value);
}

export function validateMessageHeader(value) {
  assertObject(value, "header");
  if (value.protocolVersion === undefined) fail("header.protocolVersion", "is required");
  validateProtocolVersion(value.protocolVersion);
  validateIdentifier(value.messageId, "header.messageId");
  validateIdentifier(value.correlationId ?? "", "header.correlationId", { allowEmpty: true });
  assertUint64(value.sentAtUnixMs, "header.sentAtUnixMs");
  validateIdentifier(value.senderInstanceId, "header.senderInstanceId");
  assertUint64(value.sequence, "header.sequence");
  return value;
}

export function validatePlayerSnapshot(value) {
  assertObject(value, "playerSnapshot");
  if (value.player === undefined) fail("playerSnapshot.player", "is required");
  validatePlayerDescriptor(value.player);
  assertUint64(value.revision, "playerSnapshot.revision");
  assertEnum(value.status, ENUMS.playbackStatus, "playerSnapshot.status");
  assertEnum(value.player.health, ENUMS.playerHealth, "playerSnapshot.player.health");
  if (value.track !== undefined) {
    assertObject(value.track, "playerSnapshot.track");
    assertString(value.track.provider, "playerSnapshot.track.provider", 1, VALIDATION_LIMITS.provider);
    validateIdentifier(value.track.mediaId, "playerSnapshot.track.mediaId");
    assertString(value.track.title, "playerSnapshot.track.title", 0, VALIDATION_LIMITS.title);
    if (!Array.isArray(value.track.artists) || value.track.artists.length > 64) {
      fail("playerSnapshot.track.artists", "must be an array with at most 64 items");
    }
    for (const artist of value.track.artists) {
      assertString(artist, "playerSnapshot.track.artists[]", 0, VALIDATION_LIMITS.displayName);
    }
    assertString(value.track.album, "playerSnapshot.track.album", 0, VALIDATION_LIMITS.title);
    assertUint64(value.track.durationMs, "playerSnapshot.track.durationMs");
    assertString(value.track.artworkUrl, "playerSnapshot.track.artworkUrl", 0, VALIDATION_LIMITS.url);
    assertBoolean(value.track.explicitContent, "playerSnapshot.track.explicitContent");
    assertBoolean(value.track.liked, "playerSnapshot.track.liked");
  }
  if (value.position !== undefined) {
    assertObject(value.position, "playerSnapshot.position");
    assertUint64(value.position.positionMs, "playerSnapshot.position.positionMs");
    assertUint64(value.position.measuredAtUnixMs, "playerSnapshot.position.measuredAtUnixMs");
    assertFiniteRange(value.position.playbackRate, "playerSnapshot.position.playbackRate", 0.25, 4);
    if (value.track !== undefined && value.track.durationMs > 0n && value.position.positionMs > value.track.durationMs) {
      fail("playerSnapshot.position.positionMs", "must not exceed track duration");
    }
  }
  if (value.options !== undefined) {
    assertObject(value.options, "playerSnapshot.options");
    assertFiniteRange(value.options.volume, "playerSnapshot.options.volume", 0, 1);
    assertBoolean(value.options.muted, "playerSnapshot.options.muted");
    assertBoolean(value.options.shuffle, "playerSnapshot.options.shuffle");
    assertEnum(value.options.repeatMode, ENUMS.repeatMode, "playerSnapshot.options.repeatMode");
  }
  assertUint64(value.observedAtUnixMs, "playerSnapshot.observedAtUnixMs");
  return value;
}

export function validateCommandRequest(value) {
  assertObject(value, "commandRequest");
  validateIdentifier(value.commandId, "commandRequest.commandId");
  validateIdentifier(value.targetPlayerId, "commandRequest.targetPlayerId");
  assertUint64(value.expectedRevision, "commandRequest.expectedRevision");
  assertUint64(value.deadlineUnixMs, "commandRequest.deadlineUnixMs");
  if (value.command === undefined) fail("commandRequest.command", "is required");
  assertObject(value.command, "commandRequest.command");
  assertObject(value.command.action, "commandRequest.command.action");
  const action = value.command.action;
  if (typeof action.case !== "string" || !COMMAND_CASES.has(action.case) || action.value === undefined) {
    fail("commandRequest.command.action", "must select exactly one known command");
  }
  assertObject(action.value, `commandRequest.command.${action.case}`);
  if (action.case === "seekAbsolute") assertUint64(action.value.positionMs, "commandRequest.command.seekAbsolute.positionMs");
  if (action.case === "seekRelative") assertInt64(action.value.offsetMs, "commandRequest.command.seekRelative.offsetMs");
  if (action.case === "setVolume") assertFiniteRange(action.value.volume, "commandRequest.command.setVolume.volume", 0, 1);
  if (action.case === "setMuted") assertBoolean(action.value.muted, "commandRequest.command.setMuted.muted");
  if (action.case === "setShuffle") assertBoolean(action.value.shuffle, "commandRequest.command.setShuffle.shuffle");
  if (action.case === "setRepeat") assertEnum(action.value.repeatMode, ENUMS.repeatMode, "commandRequest.command.setRepeat.repeatMode");
  if (action.case === "setLike") assertBoolean(action.value.liked, "commandRequest.command.setLike.liked");
  return value;
}

export function validateEncryptedFrame(value) {
  assertObject(value, "encryptedFrame");
  validateIdentifier(value.sessionId, "encryptedFrame.sessionId");
  assertUint64(value.sequence, "encryptedFrame.sequence");
  assertBytes(value.nonce, "encryptedFrame.nonce", 12, 12);
  assertBytes(value.ciphertext, "encryptedFrame.ciphertext", 16, VALIDATION_LIMITS.binaryField);
  assertBytes(value.associatedData, "encryptedFrame.associatedData", 0, 65_536);
  return value;
}

export function validateConnectorEnvelope(value, schema = ConnectorEnvelopeSchema) {
  return validateEnvelope(value, schema, "connectorEnvelope");
}

export function validateClientEnvelope(value, schema = ClientEnvelopeSchema) {
  return validateEnvelope(value, schema, "clientEnvelope");
}

export function validateSerializedSize(schema, message, maximum = VALIDATION_LIMITS.envelope) {
  if (!Number.isSafeInteger(maximum) || maximum < 1) throw new RangeError("maximum must be positive");
  const size = toBinary(schema, message).byteLength;
  if (size > maximum) fail("message", `serialized size ${size} exceeds ${maximum}`);
  return size;
}

export function validateProtocolError(value) {
  assertObject(value, "protocolError");
  assertEnum(value.domain, ENUMS.errorDomain, "protocolError.domain");
  assertEnum(value.code, ENUMS.errorCode, "protocolError.code");
  assertString(value.message, "protocolError.message", 0, VALIDATION_LIMITS.text);
  assertBoolean(value.retryable, "protocolError.retryable");
  assertUint64(value.retryAfterMs ?? 0n, "protocolError.retryAfterMs");
  validateStringMap(value.metadata ?? {}, "protocolError.metadata");
  return value;
}

export function validateCapabilitySet(value) {
  return normalizeCapabilitySet(value, { allowUnknown: true });
}

function validatePlayerDescriptor(value) {
  assertObject(value, "playerSnapshot.player");
  validateIdentifier(value.playerId, "playerSnapshot.player.playerId");
  assertString(value.displayName, "playerSnapshot.player.displayName", 1, VALIDATION_LIMITS.displayName);
  assertString(value.provider, "playerSnapshot.player.provider", 1, VALIDATION_LIMITS.provider);
  if (value.capabilities === undefined) fail("playerSnapshot.player.capabilities", "is required");
  validateCapabilitySet(value.capabilities);
}

function validateEnvelope(value, schema, fieldName) {
  assertObject(value, fieldName);
  if (value.header === undefined) fail(`${fieldName}.header`, "is required");
  validateMessageHeader(value.header);
  assertObject(value.payload, `${fieldName}.payload`);
  if (typeof value.payload.case !== "string" || value.payload.case.length === 0 || value.payload.value === undefined) {
    fail(`${fieldName}.payload`, "must select exactly one payload");
  }
  const payloadField = schema.fields.find(
    (field) => field.oneof === "payload" && field.name === value.payload.case,
  );
  if (payloadField === undefined) fail(`${fieldName}.payload`, "selects an unknown payload");
  assertObject(value.payload.value, `${fieldName}.payload.${value.payload.case}`);
  validateSerializedSize(schema, value);
  return value;
}

function validateStringMap(value, fieldName) {
  assertObject(value, fieldName);
  const entries = Object.entries(value);
  if (entries.length > VALIDATION_LIMITS.metadataEntries) fail(fieldName, "contains too many entries");
  for (const [key, item] of entries) {
    assertString(key, `${fieldName}.key`, 1, 128);
    assertString(item, `${fieldName}.${key}`, 0, VALIDATION_LIMITS.text);
  }
}

function assertObject(value, fieldName) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(fieldName, "must be an object");
}

function assertString(value, fieldName, minimum, maximum) {
  if (typeof value !== "string" || value.length < minimum || value.length > maximum) {
    fail(fieldName, `must contain between ${minimum} and ${maximum} characters`);
  }
}

function assertBoolean(value, fieldName) {
  if (typeof value !== "boolean") fail(fieldName, "must be a boolean");
}

function assertEnum(value, allowed, fieldName) {
  if (!Number.isInteger(value) || !allowed.has(value)) fail(fieldName, "contains an unknown enum value");
}

function assertUint64(value, fieldName) {
  if (typeof value !== "bigint" || value < 0n || value > 0xffff_ffff_ffff_ffffn) fail(fieldName, "must be an unsigned 64-bit bigint");
}

function assertInt64(value, fieldName) {
  if (typeof value !== "bigint" || value < -0x8000_0000_0000_0000n || value > 0x7fff_ffff_ffff_ffffn) fail(fieldName, "must be a signed 64-bit bigint");
}

function assertFiniteRange(value, fieldName, minimum, maximum) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) {
    fail(fieldName, `must be a finite number between ${minimum} and ${maximum}`);
  }
}

function assertBytes(value, fieldName, minimum, maximum) {
  if (!(value instanceof Uint8Array) || value.byteLength < minimum || value.byteLength > maximum) {
    fail(fieldName, `must contain between ${minimum} and ${maximum} bytes`);
  }
}

function fail(fieldName, reason) {
  throw new ValidationError(`${fieldName} ${reason}`, { metadata: { field: fieldName, reason } });
}

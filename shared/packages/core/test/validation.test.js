import assert from "node:assert/strict";
import test from "node:test";
import {
  Capability,
  ClientEnvelopeSchema,
  PlaybackStatus,
  PlayerHealth,
  RepeatMode,
  create,
} from "@ym-connect/protocol";
import {
  ValidationError,
  fromProtocolError,
  toProtocolError,
  validateClientEnvelope,
  validateCommandRequest,
  validateEncryptedFrame,
  validateIdentifier,
  validatePlayerSnapshot,
} from "../src/index.js";

const snapshot = {
  player: {
    playerId: "player:yandex:primary",
    displayName: "Yandex Music",
    provider: "yandex-music-web",
    browser: undefined,
    capabilities: {
      supported: [Capability.CAPABILITY_PLAYBACK_READ],
      required: [],
      parameters: {},
    },
    health: PlayerHealth.PLAYER_HEALTH_READY,
  },
  revision: 7n,
  status: PlaybackStatus.PLAYBACK_STATUS_PLAYING,
  track: {
    provider: "yandex-music-web",
    mediaId: "track:123",
    mediaKind: 1,
    title: "Track",
    artists: ["Artist"],
    album: "Album",
    durationMs: 180000n,
    artworkUrl: "https://example.invalid/artwork.jpg",
    explicitContent: false,
    liked: true,
  },
  position: { positionMs: 12000n, measuredAtUnixMs: 1700000000000n, playbackRate: 1 },
  options: { volume: 0.5, muted: false, shuffle: false, repeatMode: RepeatMode.REPEAT_MODE_OFF },
  observedAtUnixMs: 1700000000000n,
};

test("accepts valid player snapshots and identifiers", () => {
  assert.equal(validatePlayerSnapshot(snapshot), snapshot);
  assert.equal(validateIdentifier("player:yandex/primary"), "player:yandex/primary");
});

test("rejects impossible playback positions", () => {
  assert.throws(
    () => validatePlayerSnapshot({ ...snapshot, position: { ...snapshot.position, positionMs: 200000n } }),
    ValidationError,
  );
});

test("validates selected command payloads", () => {
  const request = {
    commandId: "command:1",
    targetPlayerId: "player:1",
    expectedRevision: 7n,
    command: { action: { case: "setVolume", value: { volume: 0.75 } } },
    deadlineUnixMs: 1700000005000n,
  };
  assert.equal(validateCommandRequest(request), request);
  assert.throws(
    () => validateCommandRequest({ ...request, command: { action: { case: "setVolume", value: { volume: 1.5 } } } }),
    ValidationError,
  );
  assert.throws(
    () => validateCommandRequest({ ...request, command: { action: { case: "unsupportedCommand", value: {} } } }),
    ValidationError,
  );
});

test("requires the fixed 96-bit ChaCha20-Poly1305 nonce", () => {
  const frame = {
    sessionId: "session:1",
    sequence: 1n,
    nonce: new Uint8Array(12),
    ciphertext: new Uint8Array(16),
    associatedData: new Uint8Array(),
  };
  assert.equal(validateEncryptedFrame(frame), frame);
  assert.throws(() => validateEncryptedFrame({ ...frame, nonce: new Uint8Array(13) }), ValidationError);
});

test("rejects unknown envelope payload cases", () => {
  const envelope = create(ClientEnvelopeSchema, {
    header: {
      protocolVersion: { major: 1, minor: 0, patch: 0 },
      messageId: "message:1",
      correlationId: "",
      sentAtUnixMs: 1700000000000n,
      senderInstanceId: "instance:1",
      sequence: 1n,
    },
    payload: { case: "ping", value: { nonce: 1n } },
  });
  assert.equal(validateClientEnvelope(envelope), envelope);
  assert.throws(
    () => validateClientEnvelope({ ...envelope, payload: { case: "unknownPayload", value: {} } }),
    ValidationError,
  );
});

test("converts typed errors to and from canonical ProtocolError messages", () => {
  const source = new ValidationError("bad input", { metadata: { field: "volume" } });
  const message = toProtocolError(source);
  assert.equal(message.domain, 2);
  assert.equal(message.code, 1);
  assert.deepEqual(message.metadata, { field: "volume" });
  const restored = fromProtocolError(message);
  assert.equal(restored.message, "bad input");
  assert.deepEqual(restored.metadata, { field: "volume" });
  assert.throws(() => new ValidationError("bad metadata", { metadata: { field: 3 } }), TypeError);
});

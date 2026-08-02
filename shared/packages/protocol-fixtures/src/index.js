import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { fromBinary, fromJson, getMessageSchema, toBinary, toJson } from "@ym-connect/protocol";

const packageDirectory = dirname(fileURLToPath(import.meta.url));
export const fixtureDirectory = resolve(packageDirectory, "../../../protocol/fixtures/v1");
export const manifestPath = resolve(fixtureDirectory, "manifest.json");
const descriptorPath = resolve(packageDirectory, "../../../protocol/descriptor/ymconnect-v1.pb");

const FIXTURE_DEFINITIONS = Object.freeze([
  {
    name: "protocol-version",
    typeName: "ymconnect.v1.ProtocolVersion",
    json: {
      major: 1,
    },
  },
  {
    name: "connector-hello",
    typeName: "ymconnect.v1.ConnectorHello",
    json: {
      browser: {
        browser_version: "140.0.7339.12",
        connector_id: "connector-chromium-profile-a",
        extension_version: "0.1.0",
        family: "BROWSER_FAMILY_CHROMIUM",
        profile_id: "profile-a",
      },
      capabilities: {
        parameters: {
          adapter_api: "1",
          provider: "yandex-music",
        },
        required: ["CAPABILITY_PLAYBACK_READ"],
        supported: [
          "CAPABILITY_PLAYBACK_READ",
          "CAPABILITY_PLAY",
          "CAPABILITY_PAUSE",
          "CAPABILITY_SEEK_ABSOLUTE",
          "CAPABILITY_NEXT",
          "CAPABILITY_PREVIOUS",
          "CAPABILITY_SET_VOLUME",
          "CAPABILITY_SET_SHUFFLE",
          "CAPABILITY_SET_REPEAT",
          "CAPABILITY_TRACK_ARTWORK",
          "CAPABILITY_TIMELINE_UPDATES",
        ],
      },
      connector_nonce: "AAECAwQFBgcICQoLDA0ODw==",
      protocol_range: {
        maximum: {
          major: 1,
          minor: 3,
        },
        minimum: {
          major: 1,
        },
      },
    },
  },
  {
    name: "player-snapshot",
    typeName: "ymconnect.v1.PlayerSnapshot",
    json: {
      observed_at_unix_ms: "1785685200123",
      options: {
        repeat_mode: "REPEAT_MODE_ALL",
        shuffle: true,
        volume: 0.72,
      },
      player: {
        browser: {
          browser_version: "140.0.7339.12",
          connector_id: "connector-chromium-profile-a",
          extension_version: "0.1.0",
          family: "BROWSER_FAMILY_CHROMIUM",
          profile_id: "profile-a",
        },
        capabilities: {
          parameters: {
            timeline_resolution_ms: "250",
          },
          supported: [
            "CAPABILITY_PLAYBACK_READ",
            "CAPABILITY_PLAY",
            "CAPABILITY_PAUSE",
            "CAPABILITY_SEEK_ABSOLUTE",
            "CAPABILITY_NEXT",
            "CAPABILITY_PREVIOUS",
            "CAPABILITY_SET_VOLUME",
            "CAPABILITY_SET_LIKE",
          ],
        },
        display_name: "Yandex Music — Profile A",
        health: "PLAYER_HEALTH_READY",
        player_id: "player:yandex-music:profile-a:tab-17",
        provider: "yandex-music",
      },
      position: {
        measured_at_unix_ms: "1785685200123",
        playback_rate: 1.0,
        position_ms: "98765",
      },
      revision: "42",
      status: "PLAYBACK_STATUS_PLAYING",
      track: {
        album: "Cross-Language Fixtures",
        artists: ["YM Connect Test Artist", "Protocol Ensemble"],
        artwork_url: "https://avatars.yandex.net/get-music-content/test/400x400",
        duration_ms: "245000",
        liked: true,
        media_id: "track-2048",
        media_kind: "MEDIA_KIND_TRACK",
        provider: "yandex-music",
        title: "Golden Vector",
      },
    },
  },
  {
    name: "command-request",
    typeName: "ymconnect.v1.CommandRequest",
    json: {
      command: {
        seek_absolute: {
          position_ms: "120000",
        },
      },
      command_id: "cmd_AQIDBAUGBwgJCgsMDQ4PEA",
      deadline_unix_ms: "1785685205000",
      expected_revision: "42",
      target_player_id: "player:yandex-music:profile-a:tab-17",
    },
  },
  {
    name: "pairing-offer",
    typeName: "ymconnect.v1.PairingOffer",
    json: {
      bridge: {
        bridge_id: "bridge-desktop-a",
        bridge_version: "0.1.0",
        certificate_fingerprint: "oH6qwaKR3CtTBkcYPk37J5sFlvlksrr+xNkUqPaJHRI=",
        display_name: "Studio Desktop",
        identity_public_key: "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
        platform: "PLATFORM_WINDOWS",
      },
      bridge_ephemeral_public_key: "Hx4dHBsaGRgXFhUUExIREA8ODQwLCgkIBwYFBAMCAQA=",
      capabilities: {
        parameters: {
          security_suite: "x25519-ed25519-hkdf-sha256-chacha20-poly1305",
        },
        required: ["CAPABILITY_ENCRYPTED_TRANSPORT", "CAPABILITY_REPLAY_PROTECTION"],
        supported: [
          "CAPABILITY_ENCRYPTED_TRANSPORT",
          "CAPABILITY_REPLAY_PROTECTION",
          "CAPABILITY_SESSION_RESUMPTION",
          "CAPABILITY_TRUST_MANAGEMENT",
          "CAPABILITY_CLIENT_REVOCATION",
          "CAPABILITY_MULTI_PLAYER",
        ],
      },
      expires_at_unix_ms: "1785685260000",
      method: "PAIRING_METHOD_QR_CODE",
      offer_nonce: "paWlpaWlpaWlpaWlpaWlpQ==",
      pairing_id: "pair_AAECAwQFBgcICQoLDA0ODw",
      protocol_range: {
        maximum: {
          major: 1,
          minor: 3,
        },
        minimum: {
          major: 1,
        },
      },
      signature:
        "WlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWg==",
    },
  },
  {
    name: "client-envelope",
    typeName: "ymconnect.v1.ClientEnvelope",
    json: {
      command_request: {
        command: {
          set_volume: {
            volume: 0.5,
          },
        },
        command_id: "cmd_AQIDBAUGBwgJCgsMDQ4PEA",
        deadline_unix_ms: "1785685205000",
        expected_revision: "42",
        target_player_id: "player:yandex-music:profile-a:tab-17",
      },
      header: {
        correlation_id: "msg_AAECAwQFBgcICQoLDA0ODw",
        message_id: "msg_EBESExQVFhcYGRobHB0eHw",
        protocol_version: {
          major: 1,
        },
        sender_instance_id: "inst_android_fixture",
        sent_at_unix_ms: "1785685201000",
        sequence: "7",
      },
    },
  },
]);

export async function loadFixtureManifest() {
  const json = await readFile(manifestPath, "utf8");
  const manifest = JSON.parse(json);
  validateManifest(manifest);
  return manifest;
}

export async function loadFixture(name) {
  const manifest = await loadFixtureManifest();
  const entry = manifest.fixtures.find((fixture) => fixture.name === name);
  if (!entry) throw new RangeError(`unknown fixture: ${name}`);
  const schema = getMessageSchema(entry.type_name);
  const [binary, jsonText] = await Promise.all([
    readFile(resolve(fixtureDirectory, entry.binary)),
    readFile(resolve(fixtureDirectory, entry.json), "utf8"),
  ]);
  const bytes = new Uint8Array(binary.buffer, binary.byteOffset, binary.byteLength);
  const json = JSON.parse(jsonText);
  return Object.freeze({ entry: Object.freeze({ ...entry }), schema, bytes, json });
}

export async function verifyFixture(name) {
  const fixture = await loadFixture(name);
  const digest = sha256(fixture.bytes);
  if (digest !== fixture.entry.binary_sha256) {
    throw new Error(
      `fixture ${name} digest mismatch: expected ${fixture.entry.binary_sha256}, received ${digest}`,
    );
  }
  if (fixture.bytes.byteLength !== fixture.entry.binary_size) {
    throw new Error(
      `fixture ${name} length mismatch: expected ${fixture.entry.binary_size}, received ${fixture.bytes.byteLength}`,
    );
  }
  const jsonDigest = sha256(new TextEncoder().encode(canonicalJsonText(fixture.json)));
  if (jsonDigest !== fixture.entry.json_sha256) {
    throw new Error(
      `fixture ${name} JSON digest mismatch: expected ${fixture.entry.json_sha256}, received ${jsonDigest}`,
    );
  }
  const binaryMessage = fromBinary(fixture.schema, fixture.bytes);
  const jsonMessage = fromJson(fixture.schema, fixture.json);
  const jsonBytes = toBinary(fixture.schema, jsonMessage);
  if (!bytesEqual(fixture.bytes, jsonBytes)) {
    throw new Error(`fixture ${name} JSON encoding does not reproduce the canonical binary`);
  }
  const canonicalJson = deepSort(toJson(fixture.schema, binaryMessage));
  if (!isDeepStrictEqual(canonicalJson, fixture.json)) {
    throw new Error(`fixture ${name} binary decoding does not reproduce the canonical JSON`);
  }
  return Object.freeze({
    name,
    typeName: fixture.entry.type_name,
    byteLength: fixture.bytes.byteLength,
    sha256: digest,
  });
}

export async function verifyAllFixtures() {
  const manifest = await loadFixtureManifest();
  return Promise.all(manifest.fixtures.map((fixture) => verifyFixture(fixture.name)));
}

export async function writeCanonicalFixtures(options = {}) {
  const outputDirectory = resolve(options.outputDirectory ?? fixtureDirectory);
  const sourceDescriptorPath = resolve(options.descriptorPath ?? descriptorPath);
  const descriptor = await readFile(sourceDescriptorPath);
  const fixtures = [];

  for (const definition of FIXTURE_DEFINITIONS) {
    const schema = getMessageSchema(definition.typeName);
    const message = fromJson(schema, definition.json);
    const bytes = toBinary(schema, message);
    const json = deepSort(toJson(schema, message));
    const jsonText = canonicalJsonText(json);
    const binaryName = `${definition.name}.bin`;
    const jsonName = `${definition.name}.json`;
    await Promise.all([
      writeFile(resolve(outputDirectory, binaryName), bytes),
      writeFile(resolve(outputDirectory, jsonName), jsonText, "utf8"),
    ]);
    fixtures.push({
      binary: binaryName,
      binary_sha256: sha256(bytes),
      binary_size: bytes.byteLength,
      json: jsonName,
      json_sha256: sha256(new TextEncoder().encode(jsonText)),
      name: definition.name,
      type_name: definition.typeName,
    });
  }

  const manifest = {
    descriptor_sha256: sha256(descriptor),
    fixtures,
    schema_version: 1,
  };
  await writeFile(resolve(outputDirectory, "manifest.json"), canonicalJsonText(manifest), "utf8");
  return Object.freeze(manifest);
}

function validateManifest(manifest) {
  if (!manifest || typeof manifest !== "object" || manifest.schema_version !== 1) {
    throw new Error("fixture manifest has an unsupported format");
  }
  if (!/^[a-f0-9]{64}$/u.test(manifest.descriptor_sha256)) {
    throw new Error("fixture manifest has an invalid descriptor digest");
  }
  if (!Array.isArray(manifest.fixtures) || manifest.fixtures.length === 0) {
    throw new Error("fixture manifest contains no fixtures");
  }
  const names = new Set();
  for (const fixture of manifest.fixtures) {
    if (typeof fixture.name !== "string" || names.has(fixture.name)) {
      throw new Error("fixture manifest contains an invalid or duplicate name");
    }
    names.add(fixture.name);
    for (const field of ["type_name", "binary", "json", "binary_sha256", "json_sha256"]) {
      if (typeof fixture[field] !== "string" || fixture[field].length === 0) {
        throw new Error(`fixture ${fixture.name} has an invalid ${field}`);
      }
    }
    if (
      !/^[a-f0-9]{64}$/u.test(fixture.binary_sha256) ||
      !/^[a-f0-9]{64}$/u.test(fixture.json_sha256)
    ) {
      throw new Error(`fixture ${fixture.name} contains an invalid digest`);
    }
    if (!Number.isSafeInteger(fixture.binary_size) || fixture.binary_size < 0) {
      throw new Error(`fixture ${fixture.name} has an invalid binary size`);
    }
  }
}

function canonicalJsonText(value) {
  return `${formatCanonicalJson(deepSort(value), 0)}\n`;
}

function formatCanonicalJson(value, indentation) {
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    const compact = `[${value.map((item) => JSON.stringify(item)).join(", ")}]`;
    if (value.every((item) => item === null || typeof item !== "object")) {
      if (indentation + compact.length <= 100) return compact;
    }
    const prefix = " ".repeat(indentation + 2);
    const items = value.map((item) => `${prefix}${formatCanonicalJson(item, indentation + 2)}`);
    return `[\n${items.join(",\n")}\n${" ".repeat(indentation)}]`;
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) return "{}";
    const prefix = " ".repeat(indentation + 2);
    const properties = entries.map(
      ([key, item]) =>
        `${prefix}${JSON.stringify(key)}: ${formatCanonicalJson(item, indentation + 2)}`,
    );
    return `{\n${properties.join(",\n")}\n${" ".repeat(indentation)}}`;
  }
  return JSON.stringify(value);
}

function deepSort(value) {
  if (Array.isArray(value)) return value.map(deepSort);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, deepSort(value[key])]),
  );
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function bytesEqual(left, right) {
  if (left.byteLength !== right.byteLength) return false;
  for (let index = 0; index < left.byteLength; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

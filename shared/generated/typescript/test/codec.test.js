import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  CapabilitySetSchema,
  ClientEnvelopeSchema,
  ProtobufError,
  ProtocolVersionSchema,
  create,
  fromBinary,
  fromJson,
  getMessageSchema,
  toBinary,
  toJson,
} from "../src/index.js";

const fixtureRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../protocol/fixtures/v1");

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

test("golden fixtures round-trip without changing bytes", async () => {
  const manifest = JSON.parse(await readFile(resolve(fixtureRoot, "manifest.json"), "utf8"));
  for (const fixture of manifest.fixtures) {
    const schema = getMessageSchema(fixture.type_name);
    const binary = new Uint8Array(await readFile(resolve(fixtureRoot, fixture.binary)));
    const json = JSON.parse(await readFile(resolve(fixtureRoot, fixture.json), "utf8"));

    assert.equal(sha256(binary), fixture.binary_sha256);
    assert.deepEqual(toBinary(schema, fromBinary(schema, binary)), binary);
    assert.deepEqual(toBinary(schema, fromJson(schema, json)), binary);
    assert.deepEqual(toJson(schema, fromBinary(schema, binary)), json);
  }
});

test("map encoding is deterministic by key order", () => {
  const first = create(CapabilitySetSchema, {
    parameters: { zeta: "last", alpha: "first", middle: "center" },
  });
  const second = create(CapabilitySetSchema, {
    parameters: { middle: "center", alpha: "first", zeta: "last" },
  });
  assert.deepEqual(toBinary(CapabilitySetSchema, first), toBinary(CapabilitySetSchema, second));
});

test("unknown fields are skipped safely", () => {
  const message = create(CapabilitySetSchema, {});
  const known = toBinary(CapabilitySetSchema, message);
  const withUnknown = Uint8Array.from([...known, 0xf8, 0x07, 0x01]);
  assert.deepEqual(fromBinary(CapabilitySetSchema, withUnknown), message);
});

test("decode limit rejects oversized inputs", () => {
  assert.throws(
    () => fromBinary(CapabilitySetSchema, new Uint8Array(9), { maxBytes: 8 }),
    (error) => error instanceof ProtobufError && error.code === "MESSAGE_TOO_LARGE",
  );
});


test("JSON decoding rejects unknown fields and multiple oneof selections", () => {
  assert.throws(
    () => fromJson(ProtocolVersionSchema, { major: 1, unexpected: true }),
    (error) => error instanceof ProtobufError && error.code === "INVALID_JSON",
  );
  assert.throws(
    () => fromJson(ClientEnvelopeSchema, { ping: { nonce: "1" }, pong: { nonce: "1" } }),
    (error) => error instanceof ProtobufError && error.code === "INVALID_JSON",
  );
});

test("JSON decoding enforces integer ranges and canonical base64", () => {
  assert.throws(
    () => fromJson(ProtocolVersionSchema, { major: 4294967296 }),
    (error) => error instanceof ProtobufError && error.code === "INVALID_JSON",
  );
  const bytesSchema = getMessageSchema("ymconnect.v1.ConnectorHello");
  assert.throws(
    () => fromJson(bytesSchema, { connector_nonce: "not base64" }),
    (error) => error instanceof ProtobufError && error.code === "INVALID_JSON",
  );
});

test("binary decoding rejects 64-bit varint overflow and invalid limits", () => {
  const overflow = Uint8Array.from([0x08, ...Array(9).fill(0x80), 0x02]);
  assert.throws(
    () => fromBinary(CapabilitySetSchema, overflow),
    (error) => error instanceof ProtobufError && error.code === "INVALID_VARINT",
  );
  const oversizedTag = Uint8Array.from([0x80, 0x80, 0x80, 0x80, 0x10]);
  assert.throws(
    () => fromBinary(CapabilitySetSchema, oversizedTag),
    (error) => error instanceof ProtobufError && error.code === "INVALID_TAG",
  );
  assert.throws(() => fromBinary(CapabilitySetSchema, new Uint8Array(), { maxBytes: 0 }), RangeError);
  assert.throws(() => fromBinary(CapabilitySetSchema, new Uint8Array(), { maxDepth: -1 }), RangeError);
});

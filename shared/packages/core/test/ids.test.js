import assert from "node:assert/strict";
import test from "node:test";
import {
  assertOpaqueId,
  generateCommandId,
  generateInstanceId,
  generateMessageId,
  generateOpaqueId,
  generateSessionId,
  isOpaqueId,
} from "../src/index.js";

const deterministicCrypto = {
  getRandomValues(bytes) {
    for (let index = 0; index < bytes.length; index += 1) bytes[index] = index;
    return bytes;
  },
};

test("creates URL-safe identifiers with independent namespaces", () => {
  assert.match(generateMessageId(deterministicCrypto), /^msg_/u);
  assert.match(generateCommandId(deterministicCrypto), /^cmd_/u);
  assert.match(generateSessionId(deterministicCrypto), /^session_/u);
  assert.match(generateInstanceId(deterministicCrypto), /^instance_/u);
});

test("validates opaque identifiers", () => {
  const id = generateOpaqueId("fixture", 16, deterministicCrypto);
  assert.equal(isOpaqueId(id, "fixture"), true);
  assert.equal(assertOpaqueId(id), id);
  assert.equal(isOpaqueId("fixture_not+safe"), false);
});

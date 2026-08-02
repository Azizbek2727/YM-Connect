import assert from "node:assert/strict";
import test from "node:test";
import {
  MAX_SUPPORTED_PROTOCOL_VERSION,
  MIN_SUPPORTED_PROTOCOL_VERSION,
  PROTOCOL_VERSION,
  REPOSITORY_VERSION,
  formatProtocolVersion,
  parseProtocolVersion,
} from "../src/index.js";

test("exports canonical repository and protocol versions", () => {
  assert.equal(REPOSITORY_VERSION, "0.1.0");
  assert.deepEqual(PROTOCOL_VERSION, { major: 1, minor: 0, patch: 0 });
  assert.deepEqual(MIN_SUPPORTED_PROTOCOL_VERSION, { major: 1, minor: 0, patch: 0 });
  assert.deepEqual(MAX_SUPPORTED_PROTOCOL_VERSION, { major: 1, minor: 3, patch: 0 });
});

test("parses and formats protocol versions", () => {
  assert.deepEqual(parseProtocolVersion("12.34.56"), { major: 12, minor: 34, patch: 56 });
  assert.equal(formatProtocolVersion({ major: 12, minor: 34, patch: 56 }), "12.34.56");
  assert.throws(() => parseProtocolVersion("01.2.3"), RangeError);
});

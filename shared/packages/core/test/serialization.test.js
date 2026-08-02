import assert from "node:assert/strict";
import test from "node:test";
import { ProtocolVersionSchema, create } from "@ym-connect/protocol";
import {
  DelimitedFrameDecoder,
  FramingError,
  decodeDelimited,
  encodeDelimited,
  serializeJsonString,
} from "../src/index.js";

const message = create(ProtocolVersionSchema, { major: 1, minor: 2, patch: 3 });

test("round-trips length-delimited frames", () => {
  const frame = encodeDelimited(ProtocolVersionSchema, message);
  assert.deepEqual(decodeDelimited(ProtocolVersionSchema, frame), message);
  assert.equal(
    serializeJsonString(ProtocolVersionSchema, message),
    '{"major":1,"minor":2,"patch":3}',
  );
});

test("decodes fragmented and coalesced streams", () => {
  const first = encodeDelimited(ProtocolVersionSchema, message);
  const second = encodeDelimited(ProtocolVersionSchema, { major: 2, minor: 0, patch: 0 });
  const combined = new Uint8Array(first.length + second.length);
  combined.set(first);
  combined.set(second, first.length);
  const decoder = new DelimitedFrameDecoder();
  assert.deepEqual(decoder.decode(ProtocolVersionSchema, combined.subarray(0, 5)), []);
  assert.deepEqual(decoder.decode(ProtocolVersionSchema, combined.subarray(5)), [
    message,
    { major: 2, minor: 0, patch: 0 },
  ]);
});

test("rejects malformed frame lengths", () => {
  assert.throws(
    () => decodeDelimited(ProtocolVersionSchema, new Uint8Array([0, 0, 0, 10, 8, 1])),
    FramingError,
  );
});

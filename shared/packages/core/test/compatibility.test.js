import assert from "node:assert/strict";
import test from "node:test";
import { Capability } from "@ym-connect/protocol";
import {
  assertProtocolCompatible,
  intersectVersionRanges,
  negotiateProtocol,
  selectProtocolVersion,
} from "../src/index.js";

const localRange = {
  minimum: { major: 1, minor: 0, patch: 0 },
  maximum: { major: 1, minor: 3, patch: 0 },
};
const remoteRange = {
  minimum: { major: 1, minor: 1, patch: 0 },
  maximum: { major: 1, minor: 2, patch: 5 },
};
const capabilities = {
  supported: [Capability.CAPABILITY_PLAY],
  required: [Capability.CAPABILITY_PLAY],
  parameters: {},
};

test("selects the newest mutually supported version", () => {
  assert.deepEqual(intersectVersionRanges(localRange, remoteRange), remoteRange);
  assert.deepEqual(selectProtocolVersion(localRange, remoteRange), remoteRange.maximum);
  assert.deepEqual(assertProtocolCompatible(localRange, remoteRange), remoteRange.maximum);
});

test("marks non-overlapping major versions incompatible", () => {
  const incompatible = {
    minimum: { major: 2, minor: 0, patch: 0 },
    maximum: { major: 2, minor: 1, patch: 0 },
  };
  assert.equal(selectProtocolVersion(localRange, incompatible), undefined);
  assert.equal(negotiateProtocol(localRange, incompatible, capabilities, capabilities).compatible, false);
});

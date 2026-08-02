import assert from "node:assert/strict";
import test from "node:test";
import { Capability } from "@ym-connect/protocol";
import {
  CapabilityError,
  negotiateCapabilities,
  normalizeCapabilitySet,
  requireCapabilities,
} from "../src/index.js";

const local = {
  supported: [Capability.CAPABILITY_PLAY, Capability.CAPABILITY_PAUSE, Capability.CAPABILITY_PLAY],
  required: [Capability.CAPABILITY_PLAY],
  parameters: { codec: "protobuf", transport: "ipc" },
};
const remote = {
  supported: [Capability.CAPABILITY_PLAY, Capability.CAPABILITY_NEXT],
  required: [Capability.CAPABILITY_PLAY],
  parameters: { codec: "protobuf", transport: "lan" },
};

test("normalizes capability sets deterministically", () => {
  assert.deepEqual(normalizeCapabilitySet(local), {
    supported: [Capability.CAPABILITY_PLAY, Capability.CAPABILITY_PAUSE],
    required: [Capability.CAPABILITY_PLAY],
    parameters: { codec: "protobuf", transport: "ipc" },
  });
});

test("negotiates intersections and matching parameters", () => {
  const result = negotiateCapabilities(local, remote);
  assert.deepEqual(result.negotiated.supported, [Capability.CAPABILITY_PLAY]);
  assert.deepEqual(result.negotiated.parameters, { codec: "protobuf" });
  assert.deepEqual(result.missingRequired, []);
});

test("reports unsupported requirements", () => {
  assert.throws(
    () => requireCapabilities(remote, [Capability.CAPABILITY_SET_VOLUME]),
    (error) => error instanceof CapabilityError && error.code === 3 && error.domain === 1,
  );
});

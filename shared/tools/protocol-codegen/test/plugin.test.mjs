import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  generateLanguage,
  parseProtoSource,
} from "../src/plugin.mjs";

const source = `syntax = "proto3";
package ymconnect.test;

enum Mode {
  MODE_UNSPECIFIED = 0;
  MODE_ACTIVE = 1;
}

message Child {
  string value = 1;
}

message Sample {
  string id = 1;
  repeated Mode modes = 2;
  map<string, string> labels = 3;
  oneof payload {
    Child child = 4;
    bytes raw = 5;
  }
}
`;

test("parses canonical proto3 constructs", () => {
  const parsed = parseProtoSource(source, "sample.proto");
  assert.equal(parsed.packageName, "ymconnect.test");
  assert.deepEqual(parsed.enums[0].values, [
    { name: "MODE_UNSPECIFIED", number: 0 },
    { name: "MODE_ACTIVE", number: 1 },
  ]);
  assert.equal(parsed.messages[1].fields.length, 5);
  assert.equal(parsed.messages[1].oneofs[0], "payload");
});

test("emits deterministic TypeScript bindings", () => {
  const files = new Map([["sample.proto", parseProtoSource(source, "sample.proto")]]);
  const first = generateLanguage("typescript", files, ["sample.proto"]);
  const second = generateLanguage("typescript", files, ["sample.proto"]);
  assert.deepEqual(first, second);
  const implementation = first.find((file) => file.name.endsWith(".js"));
  const declaration = first.find((file) => file.name.endsWith(".d.ts"));
  assert.match(implementation.content, /defineMessage\("ymconnect\.test\.Sample"/u);
  assert.match(declaration.content, /case: "child"/u);
});

test("emits Rust and Kotlin surfaces", () => {
  const files = new Map([["sample.proto", parseProtoSource(source, "sample.proto")]]);
  const [rust] = generateLanguage("rust", files, ["sample.proto"]);
  const [kotlin] = generateLanguage("kotlin", files, ["sample.proto"]);
  assert.match(rust.content, /prost::Message/u);
  assert.match(rust.content, /pub mod sample/u);
  assert.match(rust.content, /prost\(btree_map = "string, string"/u);
  assert.match(rust.content, /std::collections::BTreeMap/u);
  assert.match(kotlin.content, /inline fun sample/u);
});

test("executes the cross-platform protoc plugin launcher", () => {
  const launcher = fileURLToPath(new URL("../bin/protoc-gen-ym-connect", import.meta.url));
  const result = spawnSync(process.execPath, [launcher], {
    input: Buffer.alloc(0),
    encoding: null,
  });
  assert.equal(result.status, 1);
  assert.equal(result.stderr.length, 0);
  assert.ok(result.stdout.length > 0);
});

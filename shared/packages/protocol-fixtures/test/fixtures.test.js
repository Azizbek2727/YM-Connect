import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import {
  fixtureDirectory,
  loadFixtureManifest,
  verifyAllFixtures,
  verifyFixture,
  writeCanonicalFixtures,
} from "../src/index.js";

test("manifest lists unique canonical fixtures", async () => {
  const manifest = await loadFixtureManifest();
  assert.equal(manifest.schema_version, 1);
  assert.equal(
    new Set(manifest.fixtures.map((fixture) => fixture.name)).size,
    manifest.fixtures.length,
  );
  assert.equal(manifest.fixtures.length, 6);
});

test("all fixtures pass digest and codec conformance checks", async () => {
  const results = await verifyAllFixtures();
  assert.equal(results.length, 6);
  assert.equal(
    results.every((result) => /^[a-f0-9]{64}$/u.test(result.sha256)),
    true,
  );
});

test("loads individual fixtures by stable name", async () => {
  const result = await verifyFixture("protocol-version");
  assert.equal(result.typeName, "ymconnect.v1.ProtocolVersion");
});

test("regenerates every canonical fixture byte-for-byte", async () => {
  const outputDirectory = await mkdtemp(resolve(tmpdir(), "ym-connect-fixtures-"));
  try {
    const manifest = await writeCanonicalFixtures({ outputDirectory });
    const paths = [
      "manifest.json",
      ...manifest.fixtures.flatMap((fixture) => [fixture.binary, fixture.json]),
    ];
    for (const relativePath of paths) {
      const [expected, actual] = await Promise.all([
        readFile(resolve(fixtureDirectory, relativePath)),
        readFile(resolve(outputDirectory, relativePath)),
      ]);
      assert.deepEqual(actual, expected, relativePath);
    }
  } finally {
    await rm(outputDirectory, { recursive: true, force: true });
  }
});

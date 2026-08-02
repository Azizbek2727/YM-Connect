#!/usr/bin/env node
import { createHash } from "node:crypto";
import { lstat, readFile, readdir, realpath } from "node:fs/promises";
import path from "node:path";

const directory = path.resolve(process.argv[2] ?? "release");
const manifestName = process.argv[3] ?? "SHA256SUMS";
const manifestPath = path.join(directory, manifestName);
const rootRealPath = await realpath(directory);
const manifest = await readFile(manifestPath, "utf8");
const expected = new Map();
let previous = "";

for (const [index, line] of manifest.trimEnd().split("\n").entries()) {
  const match = /^([a-f0-9]{64})  ([^\r\n]+)$/.exec(line);
  if (!match) throw new Error(`Invalid checksum line ${index + 1}`);
  const relative = match[2];
  if (
    relative.includes("\\") ||
    relative.startsWith("/") ||
    relative.split("/").includes("..")
  ) {
    throw new Error(`Unsafe checksum path: ${relative}`);
  }
  if (previous && previous.localeCompare(relative) >= 0) {
    throw new Error(`Checksum paths are not strictly sorted at line ${index + 1}`);
  }
  previous = relative;
  if (expected.has(relative)) throw new Error(`Duplicate checksum path: ${relative}`);
  expected.set(relative, match[1]);
}

async function filesUnder(current) {
  const entries = await readdir(current, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const absolute = path.join(current, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Symbolic links are not allowed in release artifacts: ${absolute}`);
    }
    if (entry.isDirectory()) files.push(...(await filesUnder(absolute)));
    else if (entry.isFile()) files.push(absolute);
    else throw new Error(`Unsupported release filesystem entry: ${absolute}`);
  }
  return files;
}

const ignored = new Set([manifestName, `${manifestName}.sig`, `${manifestName}.pem`]);
const actualFiles = (await filesUnder(directory))
  .map((file) => path.relative(directory, file).replaceAll(path.sep, "/"))
  .filter((relative) => !ignored.has(relative))
  .sort((a, b) => a.localeCompare(b));

for (const relative of actualFiles) {
  if (!expected.has(relative)) {
    throw new Error(`Release file is not listed in ${manifestName}: ${relative}`);
  }
}
for (const [relative, digest] of expected) {
  const absolute = path.join(directory, relative);
  const metadata = await lstat(absolute);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`Checksum target is not a regular file: ${relative}`);
  }
  const targetRealPath = await realpath(absolute);
  if (
    targetRealPath !== rootRealPath &&
    !targetRealPath.startsWith(`${rootRealPath}${path.sep}`)
  ) {
    throw new Error(`Checksum target escapes the release directory: ${relative}`);
  }
  const actual = createHash("sha256").update(await readFile(absolute)).digest("hex");
  if (actual !== digest) throw new Error(`Checksum mismatch for ${relative}`);
}
if (expected.size !== actualFiles.length) {
  throw new Error("Checksum manifest contains missing files");
}
process.stdout.write(`Verified ${expected.size} release files\n`);

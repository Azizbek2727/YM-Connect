#!/usr/bin/env node
import { createHash } from "node:crypto";
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

const directory = path.resolve(process.argv[2] ?? "release");
const manifestName = process.argv[3] ?? "SHA256SUMS";
const excluded = new Set([manifestName, `${manifestName}.sig`, `${manifestName}.pem`]);

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

if (!(await lstat(directory)).isDirectory()) {
  throw new Error(`Release path is not a directory: ${directory}`);
}
const files = (await filesUnder(directory))
  .filter((file) => !excluded.has(path.relative(directory, file).replaceAll(path.sep, "/")))
  .sort((a, b) => a.localeCompare(b));

if (files.length === 0) throw new Error(`No release files found in ${directory}`);

const lines = [];
for (const file of files) {
  const relative = path.relative(directory, file).replaceAll(path.sep, "/");
  if (relative.includes("\\") || /[\r\n]/.test(relative)) {
    throw new Error(`Unsafe release filename: ${relative}`);
  }
  const digest = createHash("sha256").update(await readFile(file)).digest("hex");
  lines.push(`${digest}  ${relative}`);
}
await writeFile(path.join(directory, manifestName), `${lines.join("\n")}\n`, "utf8");
process.stdout.write(`Wrote ${lines.length} checksums to ${path.join(directory, manifestName)}\n`);

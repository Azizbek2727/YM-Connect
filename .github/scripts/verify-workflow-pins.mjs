#!/usr/bin/env node
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

async function yamlFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await yamlFiles(absolute)));
    else if (entry.isFile() && /\.ya?ml$/i.test(entry.name)) output.push(absolute);
  }
  return output;
}

const violations = [];
for (const file of await yamlFiles(".github")) {
  const lines = (await readFile(file, "utf8")).split("\n");
  for (const [index, line] of lines.entries()) {
    const match = /^\s*-?\s*uses:\s*([^\s#]+)/.exec(line);
    if (!match) continue;
    const reference = match[1].replace(/^['"]|['"]$/g, "");
    if (reference.startsWith("./")) continue;
    if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_./-]+)?@[a-f0-9]{40}$/.test(reference)) {
      violations.push(`${file}:${index + 1}: ${reference}`);
      continue;
    }
    if (!/#\s*v\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?\s*$/.test(line)) {
      violations.push(
        `${file}:${index + 1}: missing immutable version annotation for ${reference}`,
      );
    }
  }
}
if (violations.length > 0) {
  throw new Error(`Mutable or invalid action references:\n${violations.join("\n")}`);
}
process.stdout.write("Verified immutable GitHub Action references\n");

#!/usr/bin/env node
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const workspace = await readFile("pnpm-workspace.yaml", "utf8");
const requiredPackages = [
  ["@bufbuild/protobuf", "2.13.0"],
  ["@bufbuild/protoc-gen-es", "2.13.0"],
];
for (const [name, version] of requiredPackages) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const declaration = new RegExp(`^[ \\t]*["']?${escaped}["']?:\\s*${version}\\s*$`, "m");
  if (!declaration.test(workspace)) {
    throw new Error(`${name} must be pinned to ${version} in pnpm-workspace.yaml`);
  }
}

async function manifestsUnder(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    if (["node_modules", ".git", "target", "build", "dist"].includes(entry.name)) continue;
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await manifestsUnder(absolute)));
    else if (entry.isFile() && entry.name === "package.json") output.push(absolute);
  }
  return output;
}

const forbidden = new Set([
  "google-protobuf",
  "protobufjs",
  "protobufjs-cli",
  "@protobuf-ts/runtime",
]);
for (const manifestPath of await manifestsUnder(".")) {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const dependencies = {
    ...manifest.dependencies,
    ...manifest.devDependencies,
    ...manifest.optionalDependencies,
    ...manifest.peerDependencies,
  };
  for (const name of forbidden) {
    if (Object.hasOwn(dependencies, name)) {
      throw new Error(`${manifestPath} uses unsupported Protobuf runtime ${name}`);
    }
  }
}
process.stdout.write("Verified official Protobuf-ES dependency policy\n");

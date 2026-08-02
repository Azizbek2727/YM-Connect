#!/usr/bin/env node
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const approved = new Map([
  ["@bufbuild/protobuf", "2.13.0"],
  ["@bufbuild/protoc-gen-es", "2.13.0"],
]);
const forbidden = new Set([
  "google-protobuf",
  "protobufjs",
  "protobufjs-cli",
  "@protobuf-ts/runtime",
  "@protobuf-ts/runtime-rpc",
]);
const ignoredDirectories = new Set(["node_modules", ".git", "target", "build", "dist"]);

async function filesUnder(directory, predicate) {
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await filesUnder(absolute, predicate)));
    else if (entry.isFile() && predicate(entry.name, absolute)) output.push(absolute);
  }
  return output;
}

function parseCatalog(workspace) {
  const catalog = new Map();
  const lines = workspace.split(/\r?\n/u);
  let inCatalog = false;
  for (const line of lines) {
    if (/^catalog:\s*$/u.test(line)) {
      inCatalog = true;
      continue;
    }
    if (inCatalog && /^\S/u.test(line)) break;
    if (!inCatalog) continue;
    const match = /^\s{2}["']?([^"':]+(?:\/[^"':]+)?)["']?:\s*["']?([^"'\s]+)["']?\s*$/u.exec(line);
    if (match) catalog.set(match[1], match[2]);
  }
  return catalog;
}

function exactVersion(name, declared, catalog, manifestPath) {
  let specification = declared;
  if (specification === "catalog:" || specification === "catalog") {
    specification = catalog.get(name);
    if (!specification) {
      throw new Error(
        `${manifestPath} references ${name} through a missing workspace catalog entry`,
      );
    }
  }
  if (specification?.startsWith("workspace:")) {
    specification = specification.slice("workspace:".length);
  }
  const expected = approved.get(name);
  if (specification !== expected) {
    throw new Error(`${manifestPath} must pin ${name} to ${expected}; found ${declared}`);
  }
}

const workspaceText = await readFile("pnpm-workspace.yaml", "utf8");
const catalog = parseCatalog(workspaceText);
for (const [name, version] of approved) {
  if (catalog.has(name) && catalog.get(name) !== version) {
    throw new Error(`pnpm-workspace.yaml must pin ${name} to ${version}`);
  }
}

const approvedByManifestDirectory = new Map();
for (const manifestPath of await filesUnder(".", (name) => name === "package.json")) {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const dependencies = {
    ...manifest.dependencies,
    ...manifest.devDependencies,
    ...manifest.optionalDependencies,
    ...manifest.peerDependencies,
  };
  const declaredApproved = new Set();
  for (const [name, specification] of Object.entries(dependencies)) {
    if (forbidden.has(name)) {
      throw new Error(`${manifestPath} uses unsupported Protobuf runtime ${name}`);
    }
    if (approved.has(name)) {
      exactVersion(name, specification, catalog, manifestPath);
      declaredApproved.add(name);
    }
  }
  approvedByManifestDirectory.set(path.dirname(path.resolve(manifestPath)), declaredApproved);
}

function approvedDependenciesFor(sourcePath) {
  let directory = path.dirname(path.resolve(sourcePath));
  while (true) {
    if (approvedByManifestDirectory.has(directory)) {
      return approvedByManifestDirectory.get(directory);
    }
    const parent = path.dirname(directory);
    if (parent === directory) return new Set();
    directory = parent;
  }
}

const lockfile = await readFile("pnpm-lock.yaml", "utf8");
for (const name of forbidden) {
  if (lockfile.includes(`${name}@`) || lockfile.includes(`/${name}/`)) {
    throw new Error(`pnpm-lock.yaml contains unsupported Protobuf runtime ${name}`);
  }
}

const sourceExtensions = new Set([".js", ".mjs", ".cjs", ".ts", ".mts", ".cts"]);
const importPattern = /(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)["']([^"']+)["']/gu;
for (const sourcePath of await filesUnder(".", (_name, absolute) =>
  sourceExtensions.has(path.extname(absolute)),
)) {
  const source = await readFile(sourcePath, "utf8");
  const declaredApproved = approvedDependenciesFor(sourcePath);
  for (const match of source.matchAll(importPattern)) {
    const imported = match[1];
    for (const name of forbidden) {
      if (imported === name || imported.startsWith(`${name}/`)) {
        throw new Error(`${sourcePath} imports unsupported Protobuf runtime ${name}`);
      }
    }
    for (const name of approved.keys()) {
      if ((imported === name || imported.startsWith(`${name}/`)) && !declaredApproved.has(name)) {
        throw new Error(`${sourcePath} imports ${name} without an exact manifest declaration`);
      }
    }
  }
}

process.stdout.write(
  "Verified dependency-free or exactly pinned official Protobuf-ES JavaScript dependencies\n",
);

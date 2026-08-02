#!/usr/bin/env node
import { constants } from "node:fs";
import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";

const root = path.resolve(process.argv[2] ?? ".");

async function exists(relativePath) {
  try {
    await access(path.join(root, relativePath), constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function containsFile(relativeDirectory, predicate) {
  const directory = path.join(root, relativeDirectory);
  try {
    const entries = await readdir(directory, { withFileTypes: true, recursive: true });
    return entries.some((entry) => entry.isFile() && predicate(entry.name));
  } catch {
    return false;
  }
}

async function extensionUsesPlaywright() {
  const configuration = await Promise.all([
    exists("extension/playwright.config.js"),
    exists("extension/playwright.config.mjs"),
    exists("extension/playwright.config.ts"),
  ]);
  if (configuration.some(Boolean)) return true;

  try {
    const manifest = JSON.parse(
      await readFile(path.join(root, "extension/package.json"), "utf8"),
    );
    const dependencies = { ...manifest.dependencies, ...manifest.devDependencies };
    return Object.hasOwn(dependencies, "@playwright/test");
  } catch {
    return false;
  }
}

const bridge =
  (await exists("bridge")) &&
  (await containsFile("bridge", (name) => name === "Cargo.toml"));
const extension = await exists("extension/package.json");
const android =
  (await exists("android/app/build.gradle.kts")) ||
  (await exists("android/app/build.gradle"));
const playwright = extension && (await extensionUsesPlaywright());
const complete = bridge && extension && android;
const status = { bridge, extension, android, playwright, complete };

if (process.env.GITHUB_OUTPUT) {
  const { appendFile } = await import("node:fs/promises");
  const output = `${Object.entries(status)
    .map(([key, value]) => `${key}=${value}`)
    .join("\n")}\n`;
  await appendFile(process.env.GITHUB_OUTPUT, output);
}

process.stdout.write(`${JSON.stringify(status, null, 2)}\n`);

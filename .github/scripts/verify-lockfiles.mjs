#!/usr/bin/env node
import { constants } from "node:fs";
import { access, readFile } from "node:fs/promises";

async function requireFile(file) {
  try {
    await access(file, constants.R_OK);
  } catch {
    throw new Error(`Required lock or verification file is missing: ${file}`);
  }
}

await requireFile("pnpm-lock.yaml");
await requireFile("Cargo.lock");
const packageManifest = JSON.parse(await readFile("package.json", "utf8"));
if (packageManifest.packageManager !== "pnpm@11.17.0") {
  throw new Error(
    `packageManager must remain pnpm@11.17.0, found ${packageManifest.packageManager}`,
  );
}
const lock = await readFile("pnpm-lock.yaml", "utf8");
if (!/^lockfileVersion:\s*['"]?9\.0['"]?\s*$/m.test(lock)) {
  throw new Error("pnpm-lock.yaml must use lockfileVersion 9.0");
}
const cargo = await readFile("Cargo.lock", "utf8");
if (!/^version\s*=\s*4\s*$/m.test(cargo)) {
  throw new Error("Cargo.lock must use lockfile format version 4");
}

const androidPresent = await access("android/app", constants.F_OK).then(
  () => true,
  () => false,
);
if (androidPresent) await requireFile("gradle/verification-metadata.xml");
const suffix = androidPresent ? " and Gradle dependency metadata" : "";
process.stdout.write(`Verified repository lockfiles${suffix}\n`);

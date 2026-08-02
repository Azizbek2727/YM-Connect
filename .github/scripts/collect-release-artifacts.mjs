#!/usr/bin/env node
import { cp, lstat, mkdir, readdir } from "node:fs/promises";
import path from "node:path";

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

const source = path.resolve(option("--source", "."));
const destination = path.resolve(option("--destination", "release"));
const platform = option("--platform", process.platform);
const validateOnly = process.argv.includes("--validate-complete");
const requireComplete = process.argv.includes("--require-complete") || validateOnly;

async function exists(file) {
  try {
    await lstat(file);
    return true;
  } catch {
    return false;
  }
}

async function allFiles(directory) {
  if (!(await exists(directory))) return [];
  const entries = await readdir(directory, { withFileTypes: true });
  const output = [];
  for (const entry of entries) {
    const absolute = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Symbolic links are not allowed in release inputs: ${absolute}`);
    }
    if (entry.isDirectory()) output.push(...(await allFiles(absolute)));
    else if (entry.isFile()) output.push(absolute);
    else throw new Error(`Unsupported release input entry: ${absolute}`);
  }
  return output;
}

const categories = {
  protocol: [
    "shared/protocol/descriptor/ymconnect-v1.pb",
    "shared/protocol/descriptor/ymconnect-v1.pb.sha256",
    "shared/protocol/fixtures/v1",
  ],
  bridge: ["bridge/dist", "bridge/installers", "target/dist", "target/release"],
  extension: ["extension/dist", "extension/artifacts", "extension/build"],
  android: ["android/app/build/outputs/apk", "android/app/build/outputs/bundle"],
};

const permittedBridge = (file) =>
  !/\.(d|rlib|rmeta|pdb|ilk|exp|lib)$/i.test(file) &&
  !file.includes(`${path.sep}deps${path.sep}`) &&
  !file.includes(`${path.sep}incremental${path.sep}`);
const copied = new Map();

if (!validateOnly) {
  await mkdir(destination, { recursive: true });
  for (const [category, roots] of Object.entries(categories)) {
    let count = 0;
    for (const relativeRoot of roots) {
      const absoluteRoot = path.join(source, relativeRoot);
      if (!(await exists(absoluteRoot))) continue;
      const rootStat = await lstat(absoluteRoot);
      if (rootStat.isSymbolicLink()) {
        throw new Error(`Symbolic links are not allowed in release inputs: ${absoluteRoot}`);
      }
      const files = rootStat.isDirectory() ? await allFiles(absoluteRoot) : [absoluteRoot];
      for (const file of files) {
        if (category === "bridge" && !permittedBridge(file)) continue;
        const relative = path.relative(source, file);
        const target = path.join(destination, platform, relative);
        await mkdir(path.dirname(target), { recursive: true });
        await cp(file, target, { force: true });
        count += 1;
      }
    }
    copied.set(category, count);
  }
}

const releaseFiles = await allFiles(destination);
const normalized = releaseFiles.map((file) =>
  path.relative(destination, file).replaceAll(path.sep, "/"),
);
const present = {
  protocol: normalized.some((file) =>
    file.includes("shared/protocol/descriptor/ymconnect-v1.pb"),
  ),
  bridge: normalized.some(
    (file) =>
      file.includes("bridge/") ||
      file.includes("target/release/") ||
      file.includes("target/dist/"),
  ),
  extension: normalized.some(
    (file) =>
      file.includes("extension/dist/") ||
      file.includes("extension/artifacts/") ||
      file.includes("extension/build/"),
  ),
  android: normalized.some((file) =>
    /android\/app\/build\/outputs\/(apk|bundle)\//.test(file),
  ),
};

if (requireComplete) {
  const missing = Object.entries(present)
    .filter(([, value]) => !value)
    .map(([name]) => name);
  if (missing.length > 0) {
    throw new Error(`Release is incomplete; missing artifact categories: ${missing.join(", ")}`);
  }
}

const result = {
  platform,
  copied: Object.fromEntries(copied),
  present,
  files: releaseFiles.length,
};
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);

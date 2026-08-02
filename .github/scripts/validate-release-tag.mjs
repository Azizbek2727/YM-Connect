#!/usr/bin/env node
import { readFile } from "node:fs/promises";

const canonical = (await readFile("VERSION", "utf8")).trim();
const packageVersion = JSON.parse(await readFile("package.json", "utf8")).version;
const cargo = await readFile("Cargo.toml", "utf8");
const workspacePackage = cargo.slice(cargo.indexOf("[workspace.package]"));
const cargoMatch = /^version\s*=\s*"([^"]+)"/m.exec(workspacePackage);
if (!cargoMatch) throw new Error("Cargo workspace version is missing");
const cargoVersion = cargoMatch[1];
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
if (!semver.test(canonical)) {
  throw new Error(`VERSION is not a release Semantic Version: ${canonical}`);
}
if (packageVersion !== canonical || cargoVersion !== canonical) {
  throw new Error(
    `Version mismatch: VERSION=${canonical}, package.json=${packageVersion}, Cargo.toml=${cargoVersion}`,
  );
}
if (!process.argv.includes("--version-only")) {
  const tag =
    process.argv.find((argument) => argument.startsWith("v")) ?? process.env.GITHUB_REF_NAME;
  if (!tag) throw new Error("A release tag is required");
  if (tag !== `v${canonical}`) throw new Error(`Tag ${tag} must equal v${canonical}`);
}
process.stdout.write(`Validated repository version ${canonical}\n`);

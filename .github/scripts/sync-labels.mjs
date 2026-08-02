#!/usr/bin/env node
import { readFile } from "node:fs/promises";

const token = process.env.GITHUB_TOKEN;
const repository = process.env.GITHUB_REPOSITORY;
if (!token || !repository) {
  throw new Error("GITHUB_TOKEN and GITHUB_REPOSITORY are required");
}
const [owner, repo] = repository.split("/");
if (!owner || !repo) throw new Error(`Invalid GITHUB_REPOSITORY: ${repository}`);
const desired = JSON.parse(await readFile(".github/labels.json", "utf8"));

async function request(method, route, body) {
  const response = await fetch(`https://api.github.com/repos/${owner}/${repo}${route}`, {
    method,
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "ym-connect-label-sync",
      ...(body ? { "Content-Type": "application/json" } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    throw new Error(`${method} ${route} failed: ${response.status} ${await response.text()}`);
  }
  return response.status === 204 ? null : response.json();
}

const existing = [];
for (let page = 1; ; page += 1) {
  const batch = await request("GET", `/labels?per_page=100&page=${page}`);
  existing.push(...batch);
  if (batch.length < 100) break;
}

const byName = new Map(existing.map((label) => [label.name.toLowerCase(), label]));
for (const label of desired) {
  const current = byName.get(label.name.toLowerCase());
  if (!current) {
    await request("POST", "/labels", label);
    process.stdout.write(`Created label ${label.name}\n`);
    continue;
  }

  const changed =
    current.color.toLowerCase() !== label.color.toLowerCase() ||
    current.description !== label.description ||
    current.name !== label.name;
  if (!changed) continue;

  await request("PATCH", `/labels/${encodeURIComponent(current.name)}`, {
    new_name: label.name,
    color: label.color,
    description: label.description,
  });
  process.stdout.write(`Updated label ${label.name}\n`);
}
process.stdout.write(`Synchronized ${desired.length} labels\n`);

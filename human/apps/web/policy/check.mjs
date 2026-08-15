import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const [manifest, lockfile, budget] = await Promise.all(
  ["package.json", "package-lock.json", "policy/runtime-dependencies.json"].map(async (path) =>
    JSON.parse(await readFile(new URL(path, root), "utf8")),
  ),
);

assert.equal(lockfile.lockfileVersion, 3, "the web lockfile must use npm lockfile version 3");
assert.deepEqual(
  manifest.dependencies,
  budget.approved,
  "a direct runtime dependency changed without dependency-budget approval",
);
assert.ok(
  Object.keys(manifest.dependencies).length <= budget.maximumDirectRuntimeDependencies,
  "the direct runtime dependency budget was exceeded",
);
assert.deepEqual(
  lockfile.packages[""].dependencies,
  manifest.dependencies,
  "the web lockfile does not pin the declared runtime dependencies",
);

const allowedLicenses = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 AND LGPL-3.0-or-later",
  "Apache-2.0 AND LGPL-3.0-or-later AND MIT",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BlueOak-1.0.0",
  "CC-BY-4.0",
  "ISC",
  "LGPL-3.0-or-later",
  "MIT",
]);

for (const [path, metadata] of Object.entries(lockfile.packages)) {
  if (path === "") {
    continue;
  }
  assert.equal(typeof metadata.version, "string", `${path} is not pinned to a version`);
  assert.ok(allowedLicenses.has(metadata.license), `${path} uses non-allowlisted license ${metadata.license}`);
}

console.log("web dependency, lockfile, license, and runtime-budget policies passed");

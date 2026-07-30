import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readdir, readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const root = resolve(process.argv[2] ?? ".");
const trackedOnly = process.argv.includes("--tracked");
const execFileAsync = promisify(execFile);
const forbiddenNames = [
  "auth.json",
  "TAURI_SIGNING_PRIVATE_KEY",
  "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
  "SMTP_PASSWORD",
  "refresh_token",
];
const privateKeyMarkers = [
  ["PRIVATE", " KEY-----"].join(""),
  ["untrusted comment: minisign encrypted", " secret key"].join(""),
];
const testCanaries = [
  "access-ticket16-command-canary",
  "account-ticket16-command-canary",
  "jwt-ticket16-command-canary",
  "access-ticket17-canary",
  "smtp-secret-canary",
  "first-secret-canary",
  "second-secret-canary",
  "auth-canary",
  "nested-auth-canary",
];

async function walk(path) {
  const info = await stat(path);
  if (info.isFile()) return [path];
  const entries = await readdir(path, { withFileTypes: true });
  const nested = await Promise.all(
    entries
      .filter(
        (entry) =>
          !entry.isSymbolicLink() &&
          ![".git", "node_modules", "target", "dist"].includes(entry.name),
      )
      .map((entry) => walk(join(path, entry.name))),
  );
  return nested.flat();
}

const files = trackedOnly
  ? (
      await execFileAsync("git", ["ls-files", "-z"], {
        cwd: root,
        encoding: "buffer",
      })
    ).stdout
      .toString("utf8")
      .split("\0")
      .filter(Boolean)
      .map((path) => join(root, path))
  : await walk(root);

for (const path of files) {
  assert.ok(
    !forbiddenNames.some((name) => path.endsWith(name)),
    `Secret-bearing file name found: ${path}`,
  );
  let bytes;
  try {
    bytes = await readFile(path);
  } catch (error) {
    if (error?.code === "ENOENT" && trackedOnly) continue;
    throw error;
  }
  if (!trackedOnly) {
    for (const canary of testCanaries) {
      assert.ok(
        !bytes.includes(Buffer.from(canary)),
        `Test secret canary found in ${path}`,
      );
    }
  }
  if (bytes.includes(0)) continue;
  const text = bytes.toString("utf8");
  for (const marker of privateKeyMarkers) {
    assert.ok(!text.includes(marker), `Private-key marker found in ${path}`);
  }
}
console.log(
  `No private-key material, auth.json, or release canary found in ${trackedOnly ? "tracked source" : root}`,
);

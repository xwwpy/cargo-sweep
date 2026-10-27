import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(import.meta.dirname, "..");
const config = JSON.parse(
  fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
);
const tag = `v${config.version}`;
const repository = process.env.GITHUB_REPOSITORY || "xwwpy/cargo-sweep";
const token = process.env.GH_TOKEN;

if (!token) {
  throw new Error("GH_TOKEN is required");
}

const headers = {
  Authorization: `Bearer ${token}`,
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
  "User-Agent": "cargo-sweep-release",
};

const releaseResponse = await fetch(
  `https://api.github.com/repos/${repository}/releases/tags/${tag}`,
  { headers },
);
if (!releaseResponse.ok) {
  throw new Error(`Unable to read release ${tag}: ${releaseResponse.status}`);
}
const release = await releaseResponse.json();

const asset = release.assets.find((item) => item.name === "latest.json");
if (!asset) {
  throw new Error("latest.json is missing from the release");
}

const manifestResponse = await fetch(asset.browser_download_url, {
  headers: { "User-Agent": "cargo-sweep-release" },
});
if (!manifestResponse.ok) {
  throw new Error(`Unable to download latest.json: ${manifestResponse.status}`);
}
const manifest = await manifestResponse.json();

// tauri-action can mangle non-ASCII release notes on Windows. Reuse the
// UTF-8 release body so the in-app updater displays Chinese correctly.
manifest.notes = release.body;

const deleteResponse = await fetch(asset.url, {
  method: "DELETE",
  headers,
});
if (!deleteResponse.ok && deleteResponse.status !== 204) {
  throw new Error(`Unable to replace latest.json: ${deleteResponse.status}`);
}

const json = JSON.stringify(manifest, null, 2);
const uploadResponse = await fetch(
  `https://uploads.github.com/repos/${repository}/releases/${release.id}/assets?name=latest.json`,
  {
    method: "POST",
    headers: {
      ...headers,
      "Content-Type": "application/json",
      "Content-Length": Buffer.byteLength(json).toString(),
    },
    body: json,
  },
);
if (!uploadResponse.ok) {
  throw new Error(`Unable to upload latest.json: ${uploadResponse.status}`);
}

console.log(`Updated latest.json notes for ${tag}`);

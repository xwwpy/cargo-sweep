import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(import.meta.dirname, "..");
const config = JSON.parse(
  fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
);
const version = config.version;
const tag = `v${version}`;
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
const assets = release.assets;

function findAsset(predicate, description) {
  const asset = assets.find(predicate);
  if (!asset) {
    throw new Error(`Missing release asset: ${description}`);
  }
  return asset;
}

async function signatureFor(asset) {
  const signatureAsset = findAsset(
    (candidate) => candidate.name === `${asset.name}.sig`,
    `${asset.name}.sig`,
  );
  const response = await fetch(signatureAsset.url, {
    headers: { ...headers, Accept: "application/octet-stream" },
  });
  if (!response.ok) {
    throw new Error(
      `Unable to download signature for ${asset.name}: ${response.status}`,
    );
  }
  return (await response.text()).trim();
}

function downloadUrl(asset) {
  return `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(asset.name)}`;
}

const windows = findAsset(
  (asset) => asset.name.endsWith("_x64-setup.exe"),
  "Windows NSIS installer",
);
const mac = findAsset(
  (asset) => asset.name.endsWith(".app.tar.gz"),
  "macOS updater archive",
);
const linux = findAsset(
  (asset) => asset.name.endsWith(".AppImage.tar.gz"),
  "Linux AppImage updater archive",
);

const platforms = {};
const windowsEntry = {
  signature: await signatureFor(windows),
  url: downloadUrl(windows),
};
platforms["windows-x86_64"] = windowsEntry;
platforms["windows-x86_64-nsis"] = windowsEntry;

const macEntry = {
  signature: await signatureFor(mac),
  url: downloadUrl(mac),
};
// The release workflow publishes one universal macOS build.
platforms["darwin-aarch64"] = macEntry;
platforms["darwin-x86_64"] = macEntry;

platforms["linux-x86_64"] = {
  signature: await signatureFor(linux),
  url: downloadUrl(linux),
};

const manifest = {
  version,
  notes: release.body || `Cargo Sweep v${version}`,
  pub_date: release.published_at,
  platforms,
};

fs.writeFileSync(
  path.join(root, "latest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
  "utf8",
);

async function sha256(asset) {
  if (typeof asset.digest === "string" && asset.digest.startsWith("sha256:")) {
    return asset.digest.slice("sha256:".length);
  }
  const response = await fetch(asset.url, {
    headers: { ...headers, Accept: "application/octet-stream" },
  });
  if (!response.ok) {
    throw new Error(`Unable to download ${asset.name}: ${response.status}`);
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

const checksumAssets = assets
  .filter((asset) => !["latest.json", "SHA256SUMS.txt"].includes(asset.name))
  .sort((left, right) => left.name.localeCompare(right.name));
const checksumLines = [];
for (const asset of checksumAssets) {
  checksumLines.push(`${await sha256(asset)}  ${asset.name}`);
}
fs.writeFileSync(
  path.join(root, "SHA256SUMS.txt"),
  `${checksumLines.join("\n")}\n`,
  "utf8",
);

console.log(`Generated latest.json and SHA256SUMS.txt for ${tag}`);
console.log(`Platforms: ${Object.keys(platforms).join(", ")}`);

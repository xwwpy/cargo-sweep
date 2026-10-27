import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const version = JSON.parse(
  fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
).version;
const changelog = fs.readFileSync(path.join(root, "CHANGELOG.md"), "utf8");
const lines = changelog.split(/\r?\n/);
const headerPattern = new RegExp(
  `^## \\[${version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\]`,
);
const start = lines.findIndex((line) => headerPattern.test(line));

if (start < 0) {
  throw new Error(`CHANGELOG.md 中缺少 v${version} 的发布说明`);
}

const end = lines.findIndex((line, index) => index > start && /^## \[/.test(line));
const section = lines
  .slice(start + 1, end < 0 ? undefined : end)
  .join("\n")
  .trim();

if (!section) {
  throw new Error(`CHANGELOG.md 中 v${version} 的内容为空`);
}

const body = `### Cargo Sweep v${version}\n\n${section}\n`;
const outputPath = path.join(root, ".release-notes.md");
fs.writeFileSync(outputPath, body, "utf8");
console.log(`Prepared latest-only release notes for v${version}`);

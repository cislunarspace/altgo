#!/usr/bin/env bash
# 合并各平台打包生成的 updater JSON（如 latest.json）为一个完整的 latest.json。
# 输入参数：
#   $1 - 搜索目录（如 release-assets）
#   $2 - 输出文件路径（如 release-assets/latest.json）
#   $3 - 版本号（如 2.6.4）
#   $4 - 发布说明文件路径（可选，如 release_notes.md）

set -euo pipefail

SEARCH_DIR="${1:?缺少搜索目录}"
OUTPUT_FILE="${2:?缺少输出文件路径}"
VERSION="${3:?缺少版本号}"
NOTES_FILE="${4:-}"

node -e '
const fs = require("fs");
const path = require("path");

const searchDir = process.argv[1];
const outputFile = process.argv[2];
const version = process.argv[3];
const notesFile = process.argv[4];

let notes = "";
if (notesFile && fs.existsSync(notesFile)) {
  notes = fs.readFileSync(notesFile, "utf8");
}

function findJsonFiles(dir) {
  let results = [];
  if (!fs.existsSync(dir)) return results;
  const list = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of list) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results = results.concat(findJsonFiles(fullPath));
    } else if (entry.name === "latest.json" && fullPath !== path.resolve(outputFile)) {
      results.push(fullPath);
    }
  }
  return results;
}

const jsonFiles = findJsonFiles(searchDir);
console.log(`找到 ${jsonFiles.length} 个 platform latest.json 文件:`, jsonFiles);

const merged = {
  version: `v${version.replace(/^v/, "")}`,
  notes: notes || undefined,
  pub_date: new Date().toISOString(),
  platforms: {}
};

for (const f of jsonFiles) {
  try {
    const content = JSON.parse(fs.readFileSync(f, "utf8"));
    if (content.platforms) {
      Object.assign(merged.platforms, content.platforms);
    }
  } catch (err) {
    console.error(`解析 ${f} 失败:`, err);
  }
}

if (Object.keys(merged.platforms).length > 0) {
  fs.writeFileSync(outputFile, JSON.stringify(merged, null, 2), "utf8");
  console.log(`已成功生成合并后的 ${outputFile}，包含平台:`, Object.keys(merged.platforms));
} else {
  console.log("未发现平台更新元数据，跳过 latest.json 生成。");
}
' "${SEARCH_DIR}" "${OUTPUT_FILE}" "${VERSION}" "${NOTES_FILE}"

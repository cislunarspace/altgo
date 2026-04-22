#!/usr/bin/env bash
# 从 CHANGELOG.md 截取当前 tag 对应小节，写入 GitHub Release 正文（Markdown）。
# 依赖环境变量：GITHUB_REF_NAME（如 v2.2.4）、GITHUB_REPOSITORY（owner/name）
set -euo pipefail

OUT="${1:-release_notes.md}"
TAG="${GITHUB_REF_NAME:?missing GITHUB_REF_NAME}"
VER="${TAG#v}"
CHANGELOG="${CHANGELOG_PATH:-CHANGELOG.md}"

section=""
if [[ -f "${CHANGELOG}" ]]; then
  section="$(awk -v ver="$VER" '
    BEGIN { found=0 }
    /^## v/ {
      if (found) exit
      if ($0 ~ "^## v" ver " ") { found=1; print; next }
    }
    found { print }
  ' "${CHANGELOG}")"
fi

{
  echo "# altgo ${TAG}"
  echo
  if [[ -n "${section}" ]]; then
    echo "${section}"
  else
    echo "本版本在仓库 [CHANGELOG.md](https://github.com/${GITHUB_REPOSITORY}/blob/master/CHANGELOG.md) 中暂无独立小节。"
    echo
    echo "安装请下载本页 **Assets** 中的 \`.deb\` / \`.msi\`，并核对 \`checksums.txt\`。"
  fi

  prev_tag="$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || true)"
  if [[ -n "${prev_tag}" ]]; then
    echo
    echo "---"
    echo
    echo "**与 ${prev_tag} 以来的提交对比：** https://github.com/${GITHUB_REPOSITORY}/compare/${prev_tag}...${TAG}"
  fi
} >"${OUT}"

# Spec: 自动从 Conventional Commits 生成 CHANGELOG

## 概述

**问题：** 手动维护 `## Unreleased` 小节，每次发布前需同步 commits 与 CHANGELOG，易出错且重复。

**目标：** 保留 CHANGELOG.md 为人类可读历史记录，通过 CI 自动从 commits 生成 `## Unreleased` 内容，消除手动编辑。

## 架构

```
push 到 master：
  CI (tests) → git-cliff → 更新 ## Unreleased → git commit+push

push tag (v*)：
  release.yml → extract-release-notes.sh → GitHub Release
```

## 工具选型

**git-cliff** — Rust 原生、单二进制、快速、Jinja 模板高度可配置，GitHub Action 支持良好。

## 文件变更

### 新增：cliff.toml

git-cliff 配置文件，放置于仓库根目录。

配置内容：
- conventional commit 类型 → CHANGELOG 小节映射
- Jinja 模板匹配现有格式（`### Features`、`### Bug Fixes` 等）
- commit 信息清理规则

### 新增：.github/workflows/cliff.yml

可选独立 workflow，或合并入 ci.yml：
```yaml
- name: Generate CHANGELOG
  uses: orhun/git-cliff-action@v2
  with:
    config: cliff.toml
    workflow: ci
```

若独立，建议 trigger 为 `push: branches: [master]` 且仅在 tests 通过后运行。

### 修改：.github/workflows/ci.yml

在 test 步骤之后添加 git-cliff 调用。

### 修改：CHANGELOG.md

`## Unreleased` 小节由 CI 自动填充内容，标题保留不再手动编辑。

## 类型映射

| Conventional Commit 类型 | CHANGELOG 小节 |
|------------------------|----------------|
| feat | Features |
| fix | Bug Fixes |
| docs | Documentation |
| chore | Maintenance |
| perf | Performance |
| ci | CI / Release |
| refactor | Refactoring |
| style | Style |

## 示例输出

**输入 commits：**
```
feat: add transcription history
fix: resolve memory leak
ci: add release automation
docs: update README
```

**生成的 ## Unreleased：**
```markdown
## Unreleased

### Features
- add transcription history

### Bug Fixes
- resolve memory leak

### CI / Release
- add release automation

### Documentation
- update README
```

## 发布流程

1. 按规范编写 commits（`feat:`、`fix:`、`docs:` 等）
2. Push 到 master
3. CI：运行 tests → git-cliff 更新 CHANGELOG.md → 提交并推送
4. 准备发布：更新 Cargo.toml 版本号 → 创建并推送 tag（`v*`）
5. release.yml 触发 → extract-release-notes.sh 提取 CHANGELOG 小节 → GitHub Release

## 错误处理

- git-cliff 失败 → CI 失败，不合并有问题的 changelog 生成
- 无新 commits → `## Unreleased` 内容为空（正常）
- tag 之间无 commits → 空小节，release 时 extract 脚本处理

## 本地测试

```bash
cargo install git-cliff
git cliff --unreleased --tag v2.2.5
```

## 未纳入范围

- pre-commit hook（本地自动运行 git-cliff）
- breaking changes 处理
- 其他平台（AppImage、Homebrew 等）发布

## 验收标准

- [ ] `cliff.toml` 存在且配置正确
- [ ] CI 能成功运行 git-cliff 并更新 CHANGELOG.md
- [ ] `## Unreleased` 内容按类型分组且格式正确
- [ ] release workflow 正确使用更新后的 CHANGELOG 小节
- [ ] 无需手动编辑 `## Unreleased` 即可完成发布

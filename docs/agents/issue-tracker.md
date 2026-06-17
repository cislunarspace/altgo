# Issue Tracker：GitHub

本仓库的 issue 和 PRD 以 GitHub issue 的形式存在。所有操作都使用 `gh` CLI。

## 约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。多行正文请使用 heredoc。
- **查看 issue**：`gh issue view <number> --comments`，并通过 `jq` 过滤评论，同时拉取 labels。
- **列出 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，并视情况添加 `--label` 和 `--state` 过滤条件。
- **评论 issue**：`gh issue comment <number> --body "..."`
- **添加 / 移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库可从 `git remote -v` 推断 —— 在克隆目录中运行 `gh` 会自动识别。

## 当技能说 "publish to the issue tracker"

创建一个 GitHub issue。

## 当技能说 "fetch the relevant ticket"

运行 `gh issue view <number> --comments`。

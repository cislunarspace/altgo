# 领域文档

工程技能在探索代码库时应如何使用本仓库的领域文档。

# Domain Docs

How engineering skills should use this repository's domain docs when
exploring the codebase.

## 探索前请先读这些

- 仓库根目录下的 **`CONTEXT.md`**，或者
- 如果存在，仓库根目录下的 **`CONTEXT-MAP.md`** —— 它会指向每个上下文对应的 `CONTEXT.md`。阅读与主题相关的每一个。
- **`docs/adr/`** —— 阅读与你将要修改的区域相关的 ADR。在多上下文仓库中，还要检查 `src/<context>/docs/adr/` 下的上下文范围内决策。

如果这些文件不存在，**静默继续**。不要指出它们缺失，也不要建议预先创建。生产者技能（`/grill-with-docs`）会在术语或决策实际被确定时惰性地创建它们。

## Read These Before Exploring

- **`CONTEXT.md`** at the repository root, or
- if present, **`CONTEXT-MAP.md`** at the repository root — it points to the `CONTEXT.md` for each context. Read every one relevant to your topic.
- **`docs/adr/`** — read the ADRs related to the area you are about to modify. In multi-context repositories, also check context-scoped decisions under `src/<context>/docs/adr/`.

If these files do not exist, **continue silently**. Do not point out that they are missing and do not suggest creating them ahead of time. The producer skill (`/grill-with-docs`) creates them lazily, when a term or decision is actually settled.

## 文件结构

单上下文仓库（大多数仓库）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（仓库根目录存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          <- 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  <- 订货上下文相关决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## File Layout

Single-context repository (most repositories):

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

Multi-context repository (`CONTEXT-MAP.md` exists at the repo root):

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          <- system-level decisions
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  <- decisions of the ordering context
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表的词汇

当你的输出命名一个领域概念时（如 issue 标题、重构提案、假设、测试名称），使用 `CONTEXT.md` 中定义的术语。不要漂移使用术语表明确避免的同义词。

如果你需要的概念尚未出现在术语表中，这是一个信号 —— 要么你发明了项目不使用的语言（请重新考虑），要么确实存在一个缺口（记下来交给 `/grill-with-docs`）。

## Use the Glossary's Vocabulary

When your output names a domain concept (issue titles, refactor proposals, hypotheses, test names), use the terms defined in `CONTEXT.md`. Do not drift into synonyms the glossary deliberately avoids.

If a concept you need is not yet in the glossary, that is a signal — either you invented language the project does not use (reconsider), or there is a genuine gap (note it down for `/grill-with-docs`).

## 标记 ADR 冲突

如果你的输出与现有 ADR 矛盾，请显式指出，而不是静默覆盖：

> _与 ADR-0007（事件化订单）矛盾 —— 但值得重新讨论，因为…_

## Flag ADR Conflicts

If your output contradicts an existing ADR, say so explicitly instead of silently overriding it:

> _Contradicts ADR-0007 (event-sourced orders) — but worth revisiting because…_

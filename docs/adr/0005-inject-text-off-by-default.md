# 文本注入默认关闭，改为显式开关

Windows 上转写完成后的 SendInput 文本注入（把最终文本直接输入到当前焦点窗口光标处）自恢复 Windows 支持（v2.6.0）起无条件执行。往用户正在使用的输入框里打字是侵入性动作，出错代价高（历史上出现过双进程导致文本注入两次的 bug），且与"结果写入剪贴板"的默认预期冲突。决定：新增 `[output] inject_text` 布尔配置（默认 `false`）并在设置页提供开关；关闭时仅写剪贴板。注入保持一次性整段输出——现状即非流式，无需改动。

# Text Injection Off by Default, Behind an Explicit Toggle

On Windows, the SendInput text injection after transcription (typing the final text directly at the cursor of the focused window) has run unconditionally since Windows support was restored (v2.6.0). Typing into an input box the user is actively using is intrusive, costly when it goes wrong (a dual-process bug once caused the text to be injected twice), and conflicts with the default expectation that "results go to the clipboard". Decision: add a `[output] inject_text` boolean config (default `false`) with a toggle on the settings page; when off, only the clipboard is written. Injection remains one-shot full-text output — the current behavior is already non-streaming, so nothing changes there.

## Considered Options

- 默认开启、想关的人自己关：被否决。便利不应以侵入性默认值为代价；用户主动打开开关是知情选择。
- 彻底删除注入功能：被否决。一次性注入对固定工作流用户确有价值，保留为可选能力。

## Considered Options

- On by default, with those who dislike it turning it off themselves: rejected. Convenience should not come at the price of an intrusive default; a user deliberately flipping the switch is an informed choice.
- Removing injection entirely: rejected. One-shot injection is genuinely valuable to users with fixed workflows, so it stays as an optional capability.

## Consequences

- 存量 Windows 用户升级后行为变更（不再自动输入，需自行粘贴），必须在 CHANGELOG 中显著标注。
- Linux 无此能力，行为不变（仅剪贴板），设置页开关需标注平台适用性。

## Consequences

- Existing Windows users see a behavior change after upgrading (no more automatic typing; they must paste themselves) — this must be called out prominently in the CHANGELOG.
- Linux never had this capability and its behavior is unchanged (clipboard only); the settings-page toggle needs a note about platform applicability.

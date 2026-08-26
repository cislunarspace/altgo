# ADR-0003：主循环单次转写互斥

**日期**：2026-08-10
**状态**：已接受
**取代**：无

# ADR-0003: Single Transcription Mutex in the Main Loop

**Date**: 2026-08-10
**Status**: Accepted
**Supersedes**: None

## 背景

`voice_pipeline/context.rs` 的主循环是一个 `tokio::select!` 三分支：按键事件、状态机超时、停止信号。`StopRecord` 命令触发后，`handle_stop_record` 在分支内被直接 `await`，直到录音停止、转写完成、润色完成、结果分发完成，整个循环才继续监听下一次按键。

初看这里像是一个“性能 bug”：为什么不用 `tokio::spawn` 把转写任务扔出去，让主循环继续响应按键？

## Background

The main loop of `voice_pipeline/context.rs` is a three-branch `tokio::select!`: key events, state machine timeouts, and the stop signal. When the `StopRecord` command fires, `handle_stop_record` is awaited directly inside the branch; only after recording stops, transcription finishes, polishing finishes, and result dispatch completes does the loop resume listening for the next key press.

At first glance this looks like a "performance bug": why not use `tokio::spawn` to throw the transcription task out and let the main loop keep responding to key presses?

## 决策

**这是有意设计，不是 bug。**

altgo 的交互模型是“按住激活键录一句、松开后出结果”的单发操作。用户不会连续多次触发转写；即便快速连按，也只需要按顺序处理。`handle_stop_record` 在 select 分支内 `await` 保证：

- **一次只完成一次转写**；
- 上一次转写完成或整条流水线退出后，才能开始下一次；
- 不需要额外的“转写状态机”来处理并发、取消、结果叠加或竞态。

如果改成 `tokio::spawn` 并发执行，按键循环可以立即恢复，但会带来：

- 多次录音/转写结果如何排队、合并或丢弃；
- 连续按键时浮窗状态如何切换而不冲突；
- 历史记录写入顺序如何保证；
- 取消正在进行的转写是否需要引入新机制。

这些复杂度对 altgo 的“单句转写”场景没有实际收益。阻塞不是缺陷，而是正确性保证。

## Decision

**This is deliberate design, not a bug.**

altgo's interaction model is a one-shot operation: hold the activation key to record a sentence, release to get the result. Users do not trigger transcriptions many times in a row; even rapid consecutive presses only need sequential handling. Awaiting `handle_stop_record` inside the select branch guarantees:

- **Only one transcription runs at a time**;
- The next transcription can start only after the previous one finishes or the whole pipeline exits;
- No extra "transcription state machine" is needed to handle concurrency, cancellation, result stacking, or races.

Switching to concurrent execution with `tokio::spawn` would let the key loop recover immediately, but it would introduce:

- How to queue, merge, or drop results from multiple recordings/transcriptions;
- How overlay states switch without conflict during consecutive presses;
- How to guarantee history write ordering;
- Whether cancelling an in-flight transcription requires a new mechanism.

None of this complexity pays off for altgo's "single-sentence transcription" scenario. Blocking is not a defect — it is a correctness guarantee.

## 后果

- 按键事件在转写期间由无界通道缓冲，不会丢失；但下一次实际响应会延迟到当前转写结束。
- 代码更简单：没有并发转写状态机，没有额外的取消或排队逻辑。
- 这是可接受的取舍，符合 altgo 的按句触发模型。

**不要“修复”它。** 后人看到 `select` 分支内 `await` 一个长操作，容易误当成 bug。本 ADR 记录这个有意偏离。

## Consequences

- Key events are buffered by the unbounded channel while transcription runs and will not be lost; but the next actual response is delayed until the current transcription ends.
- The code stays simpler: no concurrent-transcription state machine, no extra cancellation or queueing logic.
- This is an acceptable trade-off that fits altgo's per-sentence trigger model.

**Do not "fix" it.** Someone reading the code later may mistake awaiting a long operation inside a `select` branch for a bug. This ADR records the intentional deviation.

## 备选方案

- **`tokio::spawn` 并发转写**（已否决）：引入状态机、取消、结果排序，复杂度远超收益，不符合单发交互模型。

## Alternatives Considered

- **Concurrent transcription with `tokio::spawn`** (rejected): introduces a state machine, cancellation, and result ordering — complexity far beyond the benefit, and it does not fit the one-shot interaction model.

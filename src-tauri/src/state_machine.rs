//! 状态机模块。
//!
//! 实现了一个 5 状态的按键状态机，用于区分长按和双击两种录音模式：
//!
//! - `Idle`（空闲）→ 等待按键
//! - `PotentialPress`（潜在按下）→ 按下后等待是否达到长按阈值
//! - `Recording`（录音中）→ 长按触发，松开即停止
//! - `WaitSecondClick`（等待第二次点击）→ 短按松开后等待双击
//! - `ContinuousRecording`（连续录音）→ 双击触发，再按一次停止；按住第二次时忽略系统按键连发直至松开
//!
//! 状态机通过 `tokio::select!` 同时监听按键事件和超时计时器，
//! 以实现长按阈值和双击间隔的精确控制。

use std::time::{Duration, Instant};

/// 状态机发出的命令。
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// 开始录音
    StartRecord,
    /// 停止录音
    StopRecord,
}

/// 按键事件。
#[derive(Debug)]
pub struct KeyEvent {
    /// 是否为按下事件（`true` 为按下，`false` 为松开）
    pub pressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Idle,
    PotentialPress,
    Recording,
    WaitSecondClick,
    ContinuousRecording,
}

/// 按键状态机，区分长按录音和双击连续录音。
///
/// 状态转换：
/// - Idle + 按下 → PotentialPress（启动长按计时器）
/// - PotentialPress + 计时器前松开 → WaitSecondClick
/// - PotentialPress + 计时器触发 → Recording（发出 StartRecord）
/// - Recording + 松开 → Idle（发出 StopRecord）
/// - WaitSecondClick + 按下 → ContinuousRecording（发出 StartRecord）
/// - WaitSecondClick + 计时器过期 → Idle（忽略）
/// - ContinuousRecording + 按下 → Idle（发出 StopRecord）
pub struct Machine {
    state: State,
    long_press_threshold: Duration,
    double_click_interval: Duration,
    min_press_duration: Duration,
    press_time: Option<Instant>,
    /// 连续录音开始后直到出现一次松开前，忽略再次「按下」（避免系统按键重复在按住第二次时误触发停止）。
    continuous_hold: bool,
    /// 用「再按一次」结束连续录音后，键可能仍被按住；在收到松开前忽略按下，避免误进长按检测。
    idle_suppress_press_until_release: bool,
}

impl Machine {
    /// 创建新的状态机实例。
    ///
    /// `long_press_threshold`：长按触发阈值
    /// `double_click_interval`：双击检测时间窗口
    pub fn new(
        long_press_threshold: Duration,
        double_click_interval: Duration,
        min_press_duration: Duration,
    ) -> Self {
        Self {
            state: State::Idle,
            long_press_threshold,
            double_click_interval,
            min_press_duration,
            press_time: None,
            continuous_hold: false,
            idle_suppress_press_until_release: false,
        }
    }

    /// 处理按键事件，返回需要发出的命令（如果有）。
    pub fn process(&mut self, event: KeyEvent) -> Option<Command> {
        let old_state = self.state;
        let cmd = match self.state {
            State::Idle => {
                if event.pressed && self.idle_suppress_press_until_release {
                    return None;
                }
                if !event.pressed && self.idle_suppress_press_until_release {
                    self.idle_suppress_press_until_release = false;
                    return None;
                }
                if event.pressed {
                    self.state = State::PotentialPress;
                    self.press_time = Some(Instant::now());
                }
                None
            }
            State::PotentialPress => {
                if !event.pressed {
                    // Released before long-press threshold.
                    // Reject if the press was too short — likely IME noise.
                    if let Some(pt) = self.press_time {
                        if Instant::now().duration_since(pt) < self.min_press_duration {
                            // Too quick — treat as spurious IME release.
                            // Reset press_time so poll_timeout won't fire a stale
                            // long-press timer for a key that is no longer held.
                            self.press_time = None;
                            return None;
                        }
                    }
                    self.state = State::WaitSecondClick;
                    self.press_time = Some(Instant::now());
                }
                None
            }
            State::Recording => {
                if !event.pressed {
                    self.state = State::Idle;
                    self.press_time = None;
                    Some(Command::StopRecord)
                } else {
                    None
                }
            }
            State::WaitSecondClick => {
                if event.pressed {
                    // Double click detected → continuous recording.
                    self.state = State::ContinuousRecording;
                    self.press_time = None;
                    self.continuous_hold = true;
                    Some(Command::StartRecord)
                } else {
                    None
                }
            }
            State::ContinuousRecording => {
                if event.pressed {
                    if self.continuous_hold {
                        return None;
                    }
                    self.state = State::Idle;
                    self.press_time = None;
                    self.continuous_hold = false;
                    self.idle_suppress_press_until_release = true;
                    Some(Command::StopRecord)
                } else {
                    self.continuous_hold = false;
                    None
                }
            }
        };
        if self.state != old_state {
            tracing::debug!(?old_state, new_state = ?self.state, "state transition");
        }
        cmd
    }

    /// 检查是否需要触发基于计时器的状态转换。
    pub fn poll_timeout(&mut self) -> Option<Command> {
        let now = Instant::now();

        match self.state {
            State::PotentialPress => {
                if let Some(pt) = self.press_time {
                    if now.duration_since(pt) >= self.long_press_threshold {
                        let old_state = self.state;
                        self.state = State::Recording;
                        self.press_time = None;
                        tracing::debug!(?old_state, new_state = ?self.state, "state transition (timeout)");
                        return Some(Command::StartRecord);
                    }
                }
                None
            }
            State::WaitSecondClick => {
                if let Some(pt) = self.press_time {
                    if now.duration_since(pt) >= self.double_click_interval {
                        let old_state = self.state;
                        self.state = State::Idle;
                        self.press_time = None;
                        tracing::debug!(?old_state, new_state = ?self.state, "state transition (timeout)");
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Returns the next deadline for a timer-based transition, if any.
    fn next_deadline(&self) -> Option<Instant> {
        match self.state {
            State::PotentialPress => self.press_time.map(|pt| pt + self.long_press_threshold),
            State::WaitSecondClick => self.press_time.map(|pt| pt + self.double_click_interval),
            _ => None,
        }
    }

    /// 在按键事件通道上运行状态机。
    ///
    /// 返回一个命令接收通道。当输入通道关闭时自动终止。
    pub fn run(
        self,
        mut events: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
    ) -> tokio::sync::mpsc::Receiver<Command> {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            let mut machine = self;
            loop {
                if let Some(deadline) = machine.next_deadline() {
                    tokio::select! {
                        Some(event) = events.recv() => {
                            if let Some(cmd) = machine.process(event) {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::warn!(
                                        state = ?machine.state,
                                        "command receiver dropped, shutting down state machine"
                                    );
                                    break;
                                }
                            }
                        }
                        _ = tokio::time::sleep_until(deadline.into()) => {
                            if let Some(cmd) = machine.poll_timeout() {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::warn!(
                                        state = ?machine.state,
                                        "command receiver dropped, shutting down state machine"
                                    );
                                    break;
                                }
                            }
                        }
                        else => {
                            tracing::warn!(
                                state = ?machine.state,
                                "key event channel closed (deadline branch), shutting down state machine"
                            );
                            break;
                        },
                    }
                } else {
                    // No deadline pending — wait for next event.
                    match events.recv().await {
                        Some(event) => {
                            if let Some(cmd) = machine.process(event) {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::warn!(
                                        state = ?machine.state,
                                        "command receiver dropped, shutting down state machine"
                                    );
                                    break;
                                }
                            }
                        }
                        None => {
                            tracing::warn!(
                                state = ?machine.state,
                                "key event channel closed, shutting down state machine"
                            );
                            break;
                        }
                    }
                }
            }
        });

        cmd_rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press() -> KeyEvent {
        KeyEvent { pressed: true }
    }

    fn release() -> KeyEvent {
        KeyEvent { pressed: false }
    }

    #[test]
    fn test_long_press() {
        let threshold = Duration::from_millis(50);
        let interval = Duration::from_millis(50);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        assert_eq!(sm.process(press()), None);

        std::thread::sleep(threshold + Duration::from_millis(5));
        assert_eq!(sm.poll_timeout(), Some(Command::StartRecord));

        assert_eq!(sm.process(release()), Some(Command::StopRecord));
    }

    #[test]
    fn test_double_click() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        assert_eq!(sm.process(press()), None);

        std::thread::sleep(Duration::from_millis(110));

        assert_eq!(sm.process(release()), None);

        assert_eq!(sm.process(press()), Some(Command::StartRecord));

        // Still holding (or OS key-repeat): must not stop until release.
        assert_eq!(sm.process(press()), None);
        assert_eq!(sm.process(release()), None);
        assert_eq!(sm.process(press()), Some(Command::StopRecord));
    }

    #[test]
    fn test_single_click_ignored() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        assert_eq!(sm.process(press()), None);

        std::thread::sleep(Duration::from_millis(110));

        assert_eq!(sm.process(release()), None);

        std::thread::sleep(interval + Duration::from_millis(5));
        assert_eq!(sm.poll_timeout(), None);
    }

    #[test]
    fn test_continuous_recording_stop() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        sm.process(press());
        std::thread::sleep(Duration::from_millis(110));
        sm.process(release());
        assert_eq!(sm.process(press()), Some(Command::StartRecord));

        assert_eq!(sm.process(release()), None);

        assert_eq!(sm.process(press()), Some(Command::StopRecord));
    }

    #[test]
    fn test_continuous_key_repeat_ignored() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        sm.process(press());
        std::thread::sleep(Duration::from_millis(110));
        sm.process(release());
        assert_eq!(sm.process(press()), Some(Command::StartRecord));
        assert_eq!(sm.process(press()), None);
        assert_eq!(sm.process(release()), None);
    }

    #[test]
    fn test_stop_continuous_suppresses_idle_press_until_release() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let min_press = Duration::from_millis(1);
        let mut sm = Machine::new(threshold, interval, min_press);

        sm.process(press());
        std::thread::sleep(Duration::from_millis(110));
        sm.process(release());
        assert_eq!(sm.process(press()), Some(Command::StartRecord));
        assert_eq!(sm.process(release()), None);
        assert_eq!(sm.process(press()), Some(Command::StopRecord));
        assert_eq!(sm.process(press()), None);
        assert_eq!(sm.process(release()), None);
        assert_eq!(sm.state, State::Idle);
    }

    #[test]
    fn test_spurious_quick_release_rejected() {
        let threshold = Duration::from_millis(300);
        let interval = Duration::from_millis(300);

        let mut sm = Machine {
            state: State::Idle,
            long_press_threshold: threshold,
            double_click_interval: interval,
            min_press_duration: Duration::from_millis(1),
            press_time: None,
            continuous_hold: false,
            idle_suppress_press_until_release: false,
        };

        assert_eq!(sm.process(press()), None);

        assert_eq!(sm.process(release()), None);
        assert_eq!(sm.state, State::PotentialPress);

        assert!(sm.press_time.is_none());

        assert_eq!(sm.poll_timeout(), None);
    }
}

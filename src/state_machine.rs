use std::time::{Duration, Instant};

/// Commands emitted by the state machine.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    StartRecord,
    StopRecord,
}

/// Key event from the listener.
#[derive(Debug)]
pub struct KeyEvent {
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

/// Key-press state machine that distinguishes long press from double click.
///
/// State transitions:
/// - Idle + press         → PotentialPress (start long-press timer)
/// - PotentialPress + release before timer → WaitSecondClick
/// - PotentialPress + timer fires          → Recording (emit StartRecord)
/// - Recording + release   → Idle (emit StopRecord)
/// - WaitSecondClick + press               → ContinuousRecording (emit StartRecord)
/// - WaitSecondClick + timer expires        → Idle (ignored)
/// - ContinuousRecording + press            → Idle (emit StopRecord)
pub struct Machine {
    state: State,
    long_press_threshold: Duration,
    double_click_interval: Duration,
    min_press_duration: Duration,
    press_time: Option<Instant>,
}

impl Machine {
    pub fn new(long_press_threshold: Duration, double_click_interval: Duration) -> Self {
        Self {
            state: State::Idle,
            long_press_threshold,
            double_click_interval,
            // Minimum time a key must be held before a release is treated as
            // intentional.  Filters out IME-induced release-press oscillations
            // on Windows (e.g. Chinese input methods with Right Alt).
            min_press_duration: Duration::from_millis(100),
            press_time: None,
        }
    }

    /// Process a key event, returning any command to emit.
    pub fn process(&mut self, event: KeyEvent) -> Option<Command> {
        match self.state {
            State::Idle => {
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
                    Some(Command::StartRecord)
                } else {
                    None
                }
            }
            State::ContinuousRecording => {
                if event.pressed {
                    self.state = State::Idle;
                    self.press_time = None;
                    Some(Command::StopRecord)
                } else {
                    None
                }
            }
        }
    }

    /// Check if any timer-based transition should fire.
    pub fn poll_timeout(&mut self) -> Option<Command> {
        let now = Instant::now();

        match self.state {
            State::PotentialPress => {
                if let Some(pt) = self.press_time {
                    if now.duration_since(pt) >= self.long_press_threshold {
                        self.state = State::Recording;
                        self.press_time = None;
                        return Some(Command::StartRecord);
                    }
                }
                None
            }
            State::WaitSecondClick => {
                if let Some(pt) = self.press_time {
                    if now.duration_since(pt) >= self.double_click_interval {
                        self.state = State::Idle;
                        self.press_time = None;
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

    /// Run the state machine on a channel of key events.
    ///
    /// Returns a receiver for commands. Terminates when the input channel closes.
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
                                    tracing::warn!("command receiver dropped, shutting down state machine");
                                    break;
                                }
                            }
                        }
                        _ = tokio::time::sleep_until(deadline.into()) => {
                            if let Some(cmd) = machine.poll_timeout() {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::warn!("command receiver dropped, shutting down state machine");
                                    break;
                                }
                            }
                        }
                        else => break,
                    }
                } else {
                    // No deadline pending — wait for next event.
                    match events.recv().await {
                        Some(event) => {
                            if let Some(cmd) = machine.process(event) {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::warn!(
                                        "command receiver dropped, shutting down state machine"
                                    );
                                    break;
                                }
                            }
                        }
                        None => break,
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
        let mut sm = Machine::new(threshold, interval);

        // Press → PotentialPress
        assert_eq!(sm.process(press()), None);

        // Wait for threshold → Recording
        std::thread::sleep(threshold + Duration::from_millis(5));
        assert_eq!(sm.poll_timeout(), Some(Command::StartRecord));

        // Release → StopRecord
        assert_eq!(sm.process(release()), Some(Command::StopRecord));
    }

    #[test]
    fn test_double_click() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let mut sm = Machine::new(threshold, interval);

        // Press → PotentialPress
        assert_eq!(sm.process(press()), None);

        // Hold for at least min_press_duration (100ms) before releasing
        std::thread::sleep(Duration::from_millis(110));

        // Release → WaitSecondClick
        assert_eq!(sm.process(release()), None);

        // Second press quickly → ContinuousRecording
        assert_eq!(sm.process(press()), Some(Command::StartRecord));

        // Press again → StopRecord
        assert_eq!(sm.process(press()), Some(Command::StopRecord));
    }

    #[test]
    fn test_single_click_ignored() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let mut sm = Machine::new(threshold, interval);

        // Press → PotentialPress
        assert_eq!(sm.process(press()), None);

        // Hold for at least min_press_duration before releasing
        std::thread::sleep(Duration::from_millis(110));

        // Release → WaitSecondClick
        assert_eq!(sm.process(release()), None);

        // Double-click interval expires → Idle (no command)
        std::thread::sleep(interval + Duration::from_millis(5));
        assert_eq!(sm.poll_timeout(), None);
    }

    #[test]
    fn test_continuous_recording_stop() {
        let threshold = Duration::from_millis(200);
        let interval = Duration::from_millis(200);
        let mut sm = Machine::new(threshold, interval);

        // Double click → StartRecord
        sm.process(press());
        std::thread::sleep(Duration::from_millis(110));
        sm.process(release());
        assert_eq!(sm.process(press()), Some(Command::StartRecord));

        // Release events are ignored in ContinuousRecording
        assert_eq!(sm.process(release()), None);

        // Press → StopRecord
        assert_eq!(sm.process(press()), Some(Command::StopRecord));
    }

    #[test]
    fn test_spurious_quick_release_rejected() {
        let threshold = Duration::from_millis(300);
        let interval = Duration::from_millis(300);
        let mut sm = Machine::new(threshold, interval);

        // Press → PotentialPress
        assert_eq!(sm.process(press()), None);

        // Release within 100ms (min_press_duration) → rejected, stays PotentialPress
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(sm.process(release()), None);
        assert_eq!(sm.state, State::PotentialPress);

        // press_time should be reset so no stale timer fires
        assert!(sm.press_time.is_none());

        // Without press_time, poll_timeout is a no-op
        assert_eq!(sm.poll_timeout(), None);
    }
}

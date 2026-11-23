//! Session manager: owns session records, runs backends, retries and publishes
//! events. Backends are injected so tests can run without a network server.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use hr_core::{AppError, AppResult, new_id};
use tokio::sync::{Notify, broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::frames::{Frame, FrameTransport};
use crate::plan::{EngineChoice, RdpPlan};
use crate::process::ProcessPlan;
use crate::session_plan::{SessionPlan, VncPlan};
use crate::state::{
    DisconnectReason, SessionBackend, SessionEvent, SessionSnapshot, SessionState, StateMachine,
};

pub const MAX_SESSIONS: usize = 16;
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    CtrlAltDel,
    CtrlEsc,
    AltF4,
}

#[derive(Debug, Clone)]
pub enum SessionCommand {
    Resize {
        width: u32,
        height: u32,
        scale_percent: u16,
    },
    CtrlAltDel,
    Special(SpecialKey),
    VncKey {
        keycode: u32,
        down: bool,
    },
    VncPointer {
        x: u16,
        y: u16,
        buttons: u8,
    },
    VncRefresh,
    VncText(String),
    RdpKey {
        code: String,
        key: String,
        down: bool,
    },
    RdpPointerMove {
        x: u16,
        y: u16,
    },
    RdpPointerButton {
        x: u16,
        y: u16,
        button: u8,
        down: bool,
    },
    RdpWheel {
        x: u16,
        y: u16,
        vertical: bool,
        units: i16,
    },
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    Connected,
    Frame { seq: u64, width: u32, height: u32 },
    Clipboard(String),
    Cursor(crate::state::SessionCursor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendOutcome {
    Disconnected {
        reason: DisconnectReason,
        message: Option<String>,
    },
    Failed {
        reason: DisconnectReason,
        message: String,
    },
    Cancelled,
}

impl BackendOutcome {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Failed { reason, .. } => reason.is_retryable(),
            Self::Disconnected { reason, .. } => reason.is_retryable(),
            Self::Cancelled => false,
        }
    }
}

pub struct BackendContext<'a> {
    pub plan: &'a SessionPlan,
    pub frames: Arc<FrameTransport>,
    pub events: mpsc::UnboundedSender<BackendEvent>,
    pub commands: &'a mut mpsc::UnboundedReceiver<SessionCommand>,
    pub cancel: CancelToken,
}

pub trait Backend: Send + Sync + 'static {
    fn kind(&self) -> SessionBackend;
    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>>;
}

pub struct BackendRegistry {
    embedded: Arc<dyn Backend>,
    mstsc: Arc<dyn Backend>,
    vnc: Option<Arc<dyn Backend>>,
    process: Option<Arc<dyn Backend>>,
}

impl BackendRegistry {
    pub fn new(embedded: Arc<dyn Backend>, mstsc: Arc<dyn Backend>) -> Self {
        Self {
            embedded,
            mstsc,
            vnc: None,
            process: None,
        }
    }

    pub fn with_vnc(mut self, vnc: Arc<dyn Backend>) -> Self {
        self.vnc = Some(vnc);
        self
    }

    pub fn with_process(mut self, process: Arc<dyn Backend>) -> Self {
        self.process = Some(process);
        self
    }

    fn resolve(&self, plan: &SessionPlan) -> AppResult<Arc<dyn Backend>> {
        match plan {
            SessionPlan::Rdp(plan) => Ok(match plan.engine {
                EngineChoice::Embedded => Arc::clone(&self.embedded),
                EngineChoice::ExternalMstsc => Arc::clone(&self.mstsc),
            }),
            SessionPlan::Vnc(_) => self
                .vnc
                .clone()
                .ok_or_else(|| AppError::unsupported("the VNC backend is not installed")),
            SessionPlan::Process(_) => self
                .process
                .clone()
                .ok_or_else(|| AppError::unsupported("the process backend is not installed")),
        }
    }
}

#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        // `notify_one` stores a permit, so a waiter that observes `false` just
        // before this call still completes instead of hanging.
        self.notify.notify_one();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        while !self.is_cancelled() {
            self.notify.notified().await;
        }
    }
}

struct SessionSlot {
    snapshot: Mutex<SessionSnapshot>,
    machine: Mutex<StateMachine>,
    frames: Arc<FrameTransport>,
    cancel: CancelToken,
    task: Mutex<Option<JoinHandle<()>>>,
    commands: mpsc::UnboundedSender<SessionCommand>,
    plan: SessionPlan,
}

impl SessionSlot {
    fn snapshot(&self) -> SessionSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    fn update<F: FnOnce(&mut SessionSnapshot)>(&self, update: F) {
        let mut guard = self
            .snapshot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        update(&mut guard);
    }

    fn abort_task(&self) {
        if let Some(task) = self
            .task
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
        {
            task.abort();
        }
    }
}

pub struct SessionManager {
    inner: Arc<Inner>,
}

struct Inner {
    sessions: Mutex<HashMap<String, Arc<SessionSlot>>>,
    events: broadcast::Sender<SessionEvent>,
    backends: BackendRegistry,
    max_sessions: usize,
}

impl SessionManager {
    pub fn new(backends: BackendRegistry) -> Self {
        Self::with_limit(backends, MAX_SESSIONS)
    }

    pub fn with_limit(backends: BackendRegistry, max_sessions: usize) -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                sessions: Mutex::new(HashMap::new()),
                events,
                backends,
                max_sessions: max_sessions.max(1),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.inner.events.subscribe()
    }

    pub fn list(&self) -> Vec<SessionSnapshot> {
        let sessions = self
            .inner
            .sessions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut result: Vec<SessionSnapshot> =
            sessions.values().map(|slot| slot.snapshot()).collect();
        result
            .sort_by(|left, right| (left.started_at, &left.id).cmp(&(right.started_at, &right.id)));
        result
    }

    pub fn snapshot(&self, id: &str) -> AppResult<SessionSnapshot> {
        self.slot(id).map(|slot| slot.snapshot())
    }

    pub fn frame(&self, id: &str) -> AppResult<Option<Frame>> {
        self.slot(id)?.frames.read()
    }

    pub fn frame_ring(&self, id: &str) -> AppResult<Option<String>> {
        Ok(self.slot(id)?.frames.name().map(str::to_string))
    }

    pub fn start(&self, plan: RdpPlan) -> AppResult<SessionSnapshot> {
        self.start_plan(SessionPlan::Rdp(plan))
    }

    pub fn start_vnc(&self, plan: VncPlan) -> AppResult<SessionSnapshot> {
        self.start_plan(SessionPlan::Vnc(plan))
    }

    pub fn start_process(&self, plan: ProcessPlan) -> AppResult<SessionSnapshot> {
        self.start_plan(SessionPlan::Process(plan))
    }

    pub fn start_plan(&self, plan: SessionPlan) -> AppResult<SessionSnapshot> {
        self.start_plan_with_id(plan, None)
    }

    /// Starts a plan with a caller-supplied session id so pre-connect scripts
    /// can already see `SESSION_ID` (P9-06).
    pub fn start_plan_with_id(
        &self,
        plan: SessionPlan,
        session_id: Option<String>,
    ) -> AppResult<SessionSnapshot> {
        let mut sessions = self
            .inner
            .sessions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let active = sessions
            .values()
            .filter(|slot| !slot.snapshot().state.is_terminal())
            .count();
        if active >= self.inner.max_sessions {
            return Err(AppError::validation(format!(
                "at most {} concurrent sessions are supported",
                self.inner.max_sessions
            )));
        }
        let id = session_id.unwrap_or_else(new_id);
        let backend = self.inner.backends.resolve(&plan)?;
        let ring = format!("Local\\HRemoteFrame-{id}");
        let frames = Arc::new(FrameTransport::create(&ring)?);
        let policy = plan.reconnect_policy();
        let snapshot = SessionSnapshot {
            id: id.clone(),
            server_id: plan.server_id().to_string(),
            server_name: plan.server_name().to_string(),
            backend: plan.backend_hint(),
            state: SessionState::NotInitialized,
            reason: None,
            message: None,
            attempt: 1,
            max_attempts: policy.max_attempts,
            width: plan.width(),
            height: plan.height(),
            frame_count: 0,
            started_at: now_ms(),
            connected_at: None,
            ended_at: None,
            frame_ring: frames.name().map(str::to_string),
        };
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let slot = Arc::new(SessionSlot {
            snapshot: Mutex::new(snapshot.clone()),
            machine: Mutex::new(StateMachine::default()),
            frames,
            cancel: CancelToken::default(),
            task: Mutex::new(None),
            commands: command_tx,
            plan,
        });
        sessions.insert(id.clone(), Arc::clone(&slot));
        drop(sessions);
        let handle = spawn_session(
            Arc::clone(&self.inner),
            Arc::clone(&slot),
            backend,
            command_rx,
        );
        *slot
            .task
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(handle);
        Ok(snapshot)
    }

    pub fn cancel(&self, id: &str) -> AppResult<()> {
        let slot = self.slot(id)?;
        slot.cancel.cancel();
        Ok(())
    }

    /// Starts a new generation for an existing session record. The snapshot,
    /// frame transport and plan are retained.
    pub fn reconnect(&self, id: &str) -> AppResult<SessionSnapshot> {
        let previous = self.slot(id)?;
        previous.cancel.cancel();
        previous.abort_task();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let slot = Arc::new(SessionSlot {
            snapshot: Mutex::new(previous.snapshot()),
            machine: Mutex::new(StateMachine::default()),
            frames: Arc::clone(&previous.frames),
            cancel: CancelToken::default(),
            task: Mutex::new(None),
            commands: command_tx,
            plan: previous.plan.clone(),
        });
        {
            let mut machine = slot
                .machine
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            machine.force(SessionState::Disconnected);
            machine.transition(SessionState::Reconnecting);
        }
        slot.update(|snapshot| {
            snapshot.state = SessionState::Reconnecting;
            snapshot.reason = None;
            snapshot.message = None;
            snapshot.ended_at = None;
            snapshot.attempt = 1;
            snapshot.frame_count = 0;
        });
        let backend = self.inner.backends.resolve(&slot.plan)?;
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(id.to_string(), Arc::clone(&slot));
        let handle = spawn_session(
            Arc::clone(&self.inner),
            Arc::clone(&slot),
            backend,
            command_rx,
        );
        *slot
            .task
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(handle);
        emit_state(&self.inner, &slot);
        Ok(slot.snapshot())
    }

    pub fn send_command(&self, id: &str, command: SessionCommand) -> AppResult<()> {
        let slot = self.slot(id)?;
        slot.commands
            .send(command)
            .map_err(|_| AppError::unavailable("the session is no longer accepting input"))
    }

    /// Best-effort input delivery. Pointer/keyboard events race with teardown
    /// and retries; a closed channel must not surface as a UI error.
    pub fn send_input(&self, id: &str, command: SessionCommand) -> AppResult<()> {
        let Ok(slot) = self.slot(id) else {
            return Ok(());
        };
        let _ = slot.commands.send(command);
        Ok(())
    }

    pub fn resize(&self, id: &str, width: u32, height: u32, scale_percent: u16) -> AppResult<()> {
        {
            let slot = self.slot(id)?;
            slot.update(|snapshot| {
                snapshot.width = width;
                snapshot.height = height;
            });
        }
        self.send_input(
            id,
            SessionCommand::Resize {
                width,
                height,
                scale_percent,
            },
        )
    }

    pub fn rdp_key(&self, id: &str, code: &str, key: &str, down: bool) -> AppResult<()> {
        self.send_input(
            id,
            SessionCommand::RdpKey {
                code: code.to_string(),
                key: key.to_string(),
                down,
            },
        )
    }

    pub fn rdp_pointer_move(&self, id: &str, x: u16, y: u16) -> AppResult<()> {
        self.send_input(id, SessionCommand::RdpPointerMove { x, y })
    }

    pub fn rdp_pointer_button(
        &self,
        id: &str,
        x: u16,
        y: u16,
        button: u8,
        down: bool,
    ) -> AppResult<()> {
        self.send_input(id, SessionCommand::RdpPointerButton { x, y, button, down })
    }

    pub fn rdp_wheel(&self, id: &str, x: u16, y: u16, vertical: bool, units: i16) -> AppResult<()> {
        self.send_input(
            id,
            SessionCommand::RdpWheel {
                x,
                y,
                vertical,
                units,
            },
        )
    }

    pub fn remove(&self, id: &str) -> AppResult<bool> {
        let mut sessions = self
            .inner
            .sessions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let Some(slot) = sessions.remove(id) else {
            return Ok(false);
        };
        slot.cancel.cancel();
        slot.abort_task();
        Ok(true)
    }

    pub fn shutdown(&self) {
        let sessions: Vec<Arc<SessionSlot>> = {
            let mut guard = self
                .inner
                .sessions
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            guard.drain().map(|(_, slot)| slot).collect()
        };
        for slot in sessions {
            slot.cancel.cancel();
            slot.abort_task();
        }
    }

    fn slot(&self, id: &str) -> AppResult<Arc<SessionSlot>> {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::not_found("session"))
    }
}

fn spawn_session(
    inner: Arc<Inner>,
    slot: Arc<SessionSlot>,
    backend: Arc<dyn Backend>,
    command_rx: mpsc::UnboundedReceiver<SessionCommand>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        run_session(inner, slot, backend, command_rx).await;
    })
}

async fn run_session(
    inner: Arc<Inner>,
    slot: Arc<SessionSlot>,
    backend: Arc<dyn Backend>,
    mut command_rx: mpsc::UnboundedReceiver<SessionCommand>,
) {
    let plan = slot.plan.clone();
    let policy = plan.reconnect_policy();
    let mut attempt = 1;
    loop {
        {
            let mut machine = slot
                .machine
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if machine.state() == SessionState::Reconnecting {
                machine.transition(SessionState::Connecting);
            } else {
                machine.transition(SessionState::Initializing);
                machine.transition(SessionState::Connecting);
            }
        }
        update_and_emit(&inner, &slot, |snapshot| {
            snapshot.state = SessionState::Connecting;
            snapshot.attempt = attempt;
            snapshot.message = None;
            snapshot.reason = None;
        });

        let (backend_tx, mut backend_rx) = mpsc::unbounded_channel();
        let context = BackendContext {
            plan: &plan,
            frames: Arc::clone(&slot.frames),
            events: backend_tx,
            commands: &mut command_rx,
            cancel: slot.cancel.clone(),
        };
        let mut future = backend.run(context);
        let outcome = loop {
            tokio::select! {
                _ = slot.cancel.cancelled() => break BackendOutcome::Cancelled,
                event = backend_rx.recv() => {
                    match event {
                        Some(BackendEvent::Connected) => {
                            mark_connected(&inner, &slot);
                        }
                        Some(BackendEvent::Frame { seq, width, height }) => {
                            // Frames are frequent: only touch the snapshot fields
                            // input coordinates depend on and never re-announce the
                            // connected state here (that used to emit a state event
                            // for every frame, flooding the UI and causing flicker).
                            let session_id = {
                                let mut snapshot = slot.snapshot.lock().unwrap_or_else(|poison| poison.into_inner());
                                snapshot.frame_count += 1;
                                // Track the server-negotiated size so input
                                // coordinates match the frame.
                                snapshot.width = width;
                                snapshot.height = height;
                                snapshot.id.clone()
                            };
                            let _ = inner.events.send(SessionEvent::Frame {
                                session_id,
                                seq,
                                width,
                                height,
                            });
                        }
                        Some(BackendEvent::Clipboard(text)) => {
                            let session_id = slot.snapshot().id;
                            let _ = inner.events.send(SessionEvent::Clipboard {
                                session_id,
                                text,
                            });
                        }
                        Some(BackendEvent::Cursor(cursor)) => {
                            let session_id = slot.snapshot().id;
                            let _ = inner.events.send(SessionEvent::Cursor {
                                session_id,
                                cursor,
                            });
                        }
                        None => {}
                    }
                }
                result = &mut future => break result,
            }
        };

        if matches!(outcome, BackendOutcome::Cancelled) || slot.cancel.is_cancelled() {
            finish(
                &inner,
                &slot,
                SessionState::Disconnected,
                DisconnectReason::LocalCancel,
                None,
            );
            return;
        }

        match outcome {
            BackendOutcome::Disconnected { reason, message } => {
                finish(&inner, &slot, SessionState::Disconnected, reason, message);
                return;
            }
            BackendOutcome::Failed { reason, message } => {
                if reason.is_retryable() && attempt < policy.max_attempts.max(1) {
                    let delay = policy.delay(attempt);
                    {
                        let mut machine = slot
                            .machine
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner());
                        machine.transition(SessionState::Reconnecting);
                    }
                    update_and_emit(&inner, &slot, |snapshot| {
                        snapshot.state = SessionState::Reconnecting;
                        snapshot.attempt = attempt;
                        snapshot.reason = Some(reason);
                        snapshot.message = Some(format!(
                            "{message} (retrying in {} s)",
                            delay.as_secs().max(1)
                        ));
                    });
                    tokio::time::sleep(delay).await;
                    if slot.cancel.is_cancelled() {
                        finish(
                            &inner,
                            &slot,
                            SessionState::Disconnected,
                            DisconnectReason::LocalCancel,
                            None,
                        );
                        return;
                    }
                    attempt += 1;
                    continue;
                }
                finish(&inner, &slot, SessionState::Failed, reason, Some(message));
                return;
            }
            BackendOutcome::Cancelled => {
                finish(
                    &inner,
                    &slot,
                    SessionState::Disconnected,
                    DisconnectReason::LocalCancel,
                    None,
                );
                return;
            }
        }
    }
}

fn mark_connected(inner: &Arc<Inner>, slot: &Arc<SessionSlot>) {
    {
        let mut machine = slot
            .machine
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // Backends may announce `Connected` more than once (for example before
        // every frame). Treat it as an idempotent edge, not a new transition.
        if machine.state() == SessionState::Connected {
            return;
        }
        if !machine.transition(SessionState::Connected) {
            return;
        }
    }
    update_and_emit(inner, slot, |snapshot| {
        snapshot.state = SessionState::Connected;
        snapshot.connected_at.get_or_insert_with(now_ms);
        snapshot.message = None;
        snapshot.reason = None;
    });
}

fn update_and_emit(
    inner: &Arc<Inner>,
    slot: &Arc<SessionSlot>,
    update: impl FnOnce(&mut SessionSnapshot),
) {
    {
        let mut snapshot = slot
            .snapshot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        update(&mut snapshot);
    }
    emit_state(inner, slot);
}

fn emit_state(inner: &Arc<Inner>, slot: &Arc<SessionSlot>) {
    let _ = inner.events.send(SessionEvent::State {
        session: slot.snapshot(),
    });
}

fn finish(
    inner: &Arc<Inner>,
    slot: &Arc<SessionSlot>,
    state: SessionState,
    reason: DisconnectReason,
    message: Option<String>,
) {
    {
        let mut machine = slot
            .machine
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        machine.force(state);
    }
    update_and_emit(inner, slot, |snapshot| {
        snapshot.state = state;
        snapshot.reason = Some(reason);
        snapshot.message = message.or_else(|| Some(reason.message().to_string()));
        snapshot.ended_at = Some(now_ms());
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    use hr_domain::Server;
    use tokio::sync::Notify;

    use crate::plan::ReconnectPolicy;

    #[derive(Default)]
    struct ScriptedBackend {
        behaviour: Mutex<Option<Behaviour>>,
        runs: AtomicU32,
        notify: Arc<Notify>,
    }

    #[derive(Clone)]
    enum Behaviour {
        FailThenSucceed,
        PublishFrame,
        PublishFrames(u32),
        WaitForCancel,
    }

    impl ScriptedBackend {
        fn new(behaviour: Behaviour) -> Arc<Self> {
            Arc::new(Self {
                behaviour: Mutex::new(Some(behaviour)),
                runs: AtomicU32::new(0),
                notify: Arc::new(Notify::new()),
            })
        }
    }

    impl Backend for ScriptedBackend {
        fn kind(&self) -> SessionBackend {
            SessionBackend::Synthetic
        }

        fn run<'a>(
            &'a self,
            context: BackendContext<'a>,
        ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
            Box::pin(async move {
                let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
                let behaviour = self
                    .behaviour
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .clone();
                let _ = &self.notify;
                match behaviour {
                    Some(Behaviour::FailThenSucceed) if run == 1 => BackendOutcome::Failed {
                        reason: DisconnectReason::NetworkLoss,
                        message: "simulated loss".into(),
                    },
                    Some(Behaviour::FailThenSucceed) => {
                        context.events.send(BackendEvent::Connected).ok();
                        context.cancel.cancelled().await;
                        BackendOutcome::Cancelled
                    }
                    Some(Behaviour::PublishFrame) => {
                        context
                            .frames
                            .publish(1, 1, &[1, 2, 3, 255])
                            .expect("publish");
                        context
                            .events
                            .send(BackendEvent::Frame {
                                seq: 1,
                                width: 1,
                                height: 1,
                            })
                            .ok();
                        context.events.send(BackendEvent::Connected).ok();
                        context.cancel.cancelled().await;
                        BackendOutcome::Cancelled
                    }
                    Some(Behaviour::PublishFrames(count)) => {
                        context.events.send(BackendEvent::Connected).ok();
                        for seq in 0..count {
                            context
                                .frames
                                .publish(1, 1, &[1, 2, 3, 255])
                                .expect("publish");
                            context
                                .events
                                .send(BackendEvent::Frame {
                                    seq: u64::from(seq),
                                    width: 1,
                                    height: 1,
                                })
                                .ok();
                            tokio::task::yield_now().await;
                        }
                        context.cancel.cancelled().await;
                        BackendOutcome::Cancelled
                    }
                    Some(Behaviour::WaitForCancel) | None => {
                        context.cancel.cancelled().await;
                        BackendOutcome::Cancelled
                    }
                }
            })
        }
    }

    fn plan() -> RdpPlan {
        let mut server = Server::new_rdp("Desktop", "127.0.0.1", 3390);
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.rdp_window_resize_mode = hr_domain::RdpWindowResizeMode::Fixed;
        let policy = ReconnectPolicy {
            max_attempts: 2,
            initial_delay_ms: 1,
            max_delay_ms: 2,
        };
        RdpPlan::from_server(&server, (800, 600), 100, policy).unwrap()
    }

    fn manager(backend: Arc<ScriptedBackend>) -> SessionManager {
        let mstsc = ScriptedBackend::new(Behaviour::WaitForCancel);
        SessionManager::new(BackendRegistry::new(backend, mstsc))
    }

    async fn wait_for_state(
        receiver: &mut broadcast::Receiver<SessionEvent>,
        expected: SessionState,
        timeout: Duration,
    ) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                panic!("timed out waiting for {expected:?}");
            }
            match tokio::time::timeout(remaining, receiver.recv()).await {
                Ok(Ok(SessionEvent::State { session })) if session.state == expected => return,
                Ok(Ok(_)) | Ok(Err(_)) => continue,
                Err(_) => panic!("timed out waiting for {expected:?}"),
            }
        }
    }

    #[tokio::test]
    async fn caller_supplied_session_ids_are_used_for_scripts() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let manager = manager(Arc::clone(&backend));
        let id = hr_core::new_id();
        let snapshot = manager
            .start_plan_with_id(SessionPlan::Rdp(plan()), Some(id.clone()))
            .unwrap();
        assert_eq!(snapshot.id, id);
        manager.cancel(&snapshot.id).unwrap();
    }

    #[tokio::test]
    async fn session_runs_and_can_be_cancelled() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let manager = manager(Arc::clone(&backend));
        let mut events = manager.subscribe();
        let snapshot = manager.start(plan()).unwrap();
        assert_eq!(snapshot.backend, SessionBackend::IronRdp);
        wait_for_state(
            &mut events,
            SessionState::Connecting,
            Duration::from_secs(2),
        )
        .await;
        manager.cancel(&snapshot.id).unwrap();
        wait_for_state(
            &mut events,
            SessionState::Disconnected,
            Duration::from_secs(2),
        )
        .await;
        let snapshot = manager.snapshot(&snapshot.id).unwrap();
        assert_eq!(snapshot.reason, Some(DisconnectReason::LocalCancel));
        assert!(snapshot.is_terminal());
    }

    #[tokio::test]
    async fn failed_session_retries_and_then_connects() {
        let backend = ScriptedBackend::new(Behaviour::FailThenSucceed);
        let manager = manager(Arc::clone(&backend));
        let mut events = manager.subscribe();
        let snapshot = manager.start(plan()).unwrap();
        wait_for_state(
            &mut events,
            SessionState::Reconnecting,
            Duration::from_secs(2),
        )
        .await;
        wait_for_state(&mut events, SessionState::Connected, Duration::from_secs(2)).await;
        assert_eq!(backend.runs.load(Ordering::SeqCst), 2);
        let snapshot = manager.snapshot(&snapshot.id).unwrap();
        assert_eq!(snapshot.attempt, 2);
        assert!(snapshot.connected_at.is_some());
        manager.cancel(&snapshot.id).unwrap();
    }

    #[tokio::test]
    async fn connected_is_announced_once_for_many_frames() {
        let backend = ScriptedBackend::new(Behaviour::PublishFrames(6));
        let manager = manager(Arc::clone(&backend));
        let mut events = manager.subscribe();
        let snapshot = manager.start(plan()).unwrap();
        wait_for_state(&mut events, SessionState::Connected, Duration::from_secs(2)).await;
        tokio::time::sleep(Duration::from_millis(150)).await;

        let mut extra_connected = 0;
        while let Ok(event) = events.try_recv() {
            if let SessionEvent::State { session } = event {
                if session.id == snapshot.id && session.state == SessionState::Connected {
                    extra_connected += 1;
                }
            }
        }
        assert_eq!(
            extra_connected, 0,
            "frames must not re-announce the connected state"
        );
        assert!(
            manager.snapshot(&snapshot.id).unwrap().frame_count >= 6,
            "all frames should still be delivered"
        );
        manager.cancel(&snapshot.id).unwrap();
    }

    #[tokio::test]
    async fn frames_are_stored_and_announced() {
        let backend = ScriptedBackend::new(Behaviour::PublishFrame);
        let manager = manager(Arc::clone(&backend));
        let snapshot = manager.start(plan()).unwrap();
        let frame = {
            let mut attempts = 0;
            loop {
                if let Some(frame) = manager.frame(&snapshot.id).unwrap() {
                    break frame;
                }
                attempts += 1;
                assert!(attempts < 200, "no frame published");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        assert_eq!((frame.width, frame.height), (1, 1));
        assert_eq!(frame.rgba, vec![1, 2, 3, 255]);
        assert!(manager.snapshot(&snapshot.id).unwrap().frame_count >= 1);
        manager.cancel(&snapshot.id).unwrap();
    }

    #[tokio::test]
    async fn reconnect_starts_a_new_generation() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let manager = manager(Arc::clone(&backend));
        let snapshot = manager.start(plan()).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        manager.cancel(&snapshot.id).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let reconnected = manager.reconnect(&snapshot.id).unwrap();
        assert_eq!(reconnected.id, snapshot.id);
        assert_eq!(reconnected.state, SessionState::Reconnecting);
        for _ in 0..100 {
            if manager.snapshot(&snapshot.id).unwrap().state == SessionState::Connecting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(backend.runs.load(Ordering::SeqCst) >= 2);
        manager.cancel(&snapshot.id).unwrap();
    }

    #[tokio::test]
    async fn limits_concurrent_sessions_and_removes_them() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let mstsc = ScriptedBackend::new(Behaviour::WaitForCancel);
        let registry = BackendRegistry::new(backend, mstsc);
        let manager = SessionManager::with_limit(registry, 1);
        let snapshot = manager.start(plan()).unwrap();
        assert!(manager.start(plan()).is_err());
        assert!(manager.remove(&snapshot.id).unwrap());
        assert!(manager.snapshot(&snapshot.id).is_err());
        assert!(!manager.remove(&snapshot.id).unwrap());
    }

    #[tokio::test]
    async fn input_after_teardown_is_a_noop() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let manager = manager(Arc::clone(&backend));
        let snapshot = manager.start(plan()).unwrap();
        manager.cancel(&snapshot.id).unwrap();
        assert!(manager.rdp_key(&snapshot.id, "KeyA", "a", true).is_ok());
        assert!(manager.rdp_pointer_move(&snapshot.id, 10, 10).is_ok());
        assert!(manager.rdp_pointer_move("missing-session", 10, 10).is_ok());
    }

    #[tokio::test]
    async fn shutdown_cancels_every_session() {
        let backend = ScriptedBackend::new(Behaviour::WaitForCancel);
        let manager = manager(Arc::clone(&backend));
        let snapshot = manager.start(plan()).unwrap();
        tokio::time::sleep(Duration::from_millis(30)).await;
        manager.shutdown();
        assert!(manager.list().is_empty());
        assert!(manager.snapshot(&snapshot.id).is_err());
    }
}

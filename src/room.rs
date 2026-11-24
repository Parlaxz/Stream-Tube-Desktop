//! Watch-party rooms: relay client, saved friends, invites and player sync.
//!
//! One peer hosts a room and is the authority. Guests send intents to the host;
//! the host applies them locally and broadcasts state to the room. The relay
//! (tools/relay) only routes these tiny control lines; video never passes
//! through it. Sync drives the embedded player directly, so no mpv IPC is used.
use crate::api::types::VideoItem;
use crate::player::controller;
use crate::store;
use crate::ui::{toast, toast_with_actions};
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const KEY: &str = "room_cfg";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub relay: String,
    pub token: String,
    pub name: String,
    /// this install's stable friend code
    pub me: String,
    pub friends: Vec<Friend>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Offline,
    Connecting,
    Online,
    InRoom,
}

#[derive(Clone)]
pub struct RoomInfo {
    pub id: String,
    pub host: String,
}

#[derive(Clone)]
pub struct Invite {
    pub room: String,
    pub host: String,
    pub name: String,
}

/// One wire message. `kind` is "invite", "join", "intent" (guest -> host) or
/// "state" (host -> room).
#[derive(Clone, Serialize, Deserialize)]
pub struct Sync {
    #[serde(rename = "t")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub room: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoItem>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub pos: f64,
    #[serde(default = "one")]
    pub speed: f64,
    #[serde(default)]
    pub queue: Vec<VideoItem>,
    #[serde(default)]
    pub qpos: Option<usize>,
}

fn one() -> f64 {
    1.0
}

struct Inner {
    cfg: Config,
    status: Status,
    error: Option<String>,
    online: Vec<String>,
    room: Option<RoomInfo>,
    host: bool,
    invite: Option<Invite>,
    out: Option<Sender<String>>,
    ctl: Option<TcpStream>,
    last_sent: Option<Sync>,
    /// last (time, at, speed, paused) seen from the player, to tell a seek from
    /// normal playback
    obs: Option<(f64, Instant, f64, bool)>,
    beat: Instant,
    who: Instant,
    ready: bool,
    dirty: bool,
}

impl Inner {
    fn new() -> Self {
        let mut cfg = store::kv_get(KEY).and_then(|s| serde_json::from_str::<Config>(&s).ok()).unwrap_or_default();
        if cfg.me.is_empty() {
            cfg.me = new_id();
        }
        if cfg.name.trim().is_empty() {
            cfg.name = "Me".into();
        }
        save_cfg(&cfg);
        Self {
            cfg,
            status: Status::Offline,
            error: None,
            online: Vec::new(),
            room: None,
            host: false,
            invite: None,
            out: None,
            ctl: None,
            last_sent: None,
            obs: None,
            beat: Instant::now(),
            who: Instant::now(),
            ready: false,
            dirty: false,
        }
    }
}

pub struct Room {
    inner: Mutex<Inner>,
}

impl Default for Room {
    fn default() -> Self {
        Self { inner: Mutex::new(Inner::new()) }
    }
}

impl Global for Room {}

fn new_id() -> String {
    format!("{:016x}{:016x}", rand::random::<u64>(), rand::random::<u64>())
}

fn save_cfg(c: &Config) {
    if let Ok(v) = serde_json::to_string(c) {
        store::kv_set(KEY, &v);
    }
}

/// A copy of the room state for the UI.
pub struct Snapshot {
    pub cfg: Config,
    pub status: Status,
    pub error: Option<String>,
    pub online: Vec<String>,
    pub room: Option<RoomInfo>,
    pub host: bool,
    pub invite: Option<Invite>,
}

pub fn snapshot(cx: &mut App) -> Snapshot {
    let r = cx.default_global::<Room>();
    let g = r.inner.lock().unwrap();
    Snapshot {
        cfg: g.cfg.clone(),
        status: g.status,
        error: g.error.clone(),
        online: g.online.clone(),
        room: g.room.clone(),
        host: g.host,
        invite: g.invite.clone(),
    }
}

enum Event {
    Connected(Sender<String>, TcpStream),
    Line(String),
    Down(String),
}

static EVENTS: OnceLock<futures::channel::mpsc::UnboundedSender<Event>> = OnceLock::new();

pub fn init(cx: &mut App) {
    cx.set_global(Room::default());
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let _ = EVENTS.set(tx);
    cx.spawn(async move |cx| {
        use futures::StreamExt;
        while let Some(ev) = rx.next().await {
            let _ = cx.update(|app| handle_event(app, ev));
        }
    })
    .detach();
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(Duration::from_millis(500)).await;
        let _ = cx.update(|app| tick(app));
    })
    .detach();
    let relay = { cx.default_global::<Room>().inner.lock().unwrap().cfg.relay.clone() };
    if !relay.trim().is_empty() {
        connect(cx);
    }
}

fn events() -> Option<futures::channel::mpsc::UnboundedSender<Event>> {
    EVENTS.get().cloned()
}

// ── public actions used by the room page ───────────────────────────────────

pub fn save_settings(relay: String, token: String, name: String, cx: &mut App) {
    let r = cx.default_global::<Room>();
    let mut g = r.inner.lock().unwrap();
    g.cfg.relay = relay.trim().to_string();
    g.cfg.token = token.trim().to_string();
    if !name.trim().is_empty() {
        g.cfg.name = name.trim().to_string();
    }
    save_cfg(&g.cfg);
}

pub fn connect(cx: &mut App) {
    let (relay, token, me, name) = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        (g.cfg.relay.clone(), g.cfg.token.clone(), g.cfg.me.clone(), g.cfg.name.clone())
    };
    if relay.trim().is_empty() {
        toast(cx, "Set the relay address first");
        return;
    }
    // host:port, tolerating a missing port
    let relay = if relay.contains(':') { relay } else { format!("{relay}:8930") };
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.status = Status::Connecting;
        g.error = None;
    }
    cx.refresh_windows();
    let Some(tx) = events() else { return };
    std::thread::spawn(move || net_loop(relay, token, me, name, tx));
}

pub fn disconnect(cx: &mut App) {
    let ctl = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.out = None;
        g.status = Status::Offline;
        g.room = None;
        g.host = false;
        g.ready = false;
        g.ctl.take()
    };
    if let Some(s) = ctl {
        let _ = s.shutdown(Shutdown::Both);
    }
    cx.refresh_windows();
}

pub fn add_friend(id: String, name: String, cx: &mut App) {
    let id = id.trim().to_string();
    let name = name.trim().to_string();
    if id.is_empty() {
        toast(cx, "Enter a friend code");
        return;
    }
    let dup = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.cfg.friends.iter().any(|f| f.id == id) {
            true
        } else {
            let label = if name.is_empty() { id[..id.len().min(8)].to_string() } else { name };
            g.cfg.friends.push(Friend { id, name: label });
            save_cfg(&g.cfg);
            false
        }
    };
    if dup {
        toast(cx, "Already in your friends");
    } else {
        cx.refresh_windows();
    }
}

pub fn remove_friend(id: String, cx: &mut App) {
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.cfg.friends.retain(|f| f.id != id);
        save_cfg(&g.cfg);
    }
    cx.refresh_windows();
}

pub fn create_room(cx: &mut App) {
    let ok = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.status != Status::Online {
            false
        } else {
            let me = g.cfg.me.clone();
            let id = format!("{:08x}", rand::random::<u32>());
            g.room = Some(RoomInfo { id: id.clone(), host: me });
            g.host = true;
            g.status = Status::InRoom;
            g.ready = true;
            g.last_sent = None;
            g.dirty = true;
            send_now(&g, &format!("JOIN {id}"));
            true
        }
    };
    if !ok {
        toast(cx, "Connect to the relay first");
        return;
    }
    toast(cx, "Room created. Invite a friend.");
    cx.refresh_windows();
}

pub fn join_room(cx: &mut App, room: String, host: String) {
    let ok = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.status == Status::Offline {
            false
        } else {
            g.room = Some(RoomInfo { id: room.clone(), host: host.clone() });
            g.host = false;
            g.status = Status::InRoom;
            g.ready = false;
            g.last_sent = None;
            g.dirty = false;
            g.invite = None;
            send_now(&g, &format!("JOIN {room}"));
            let name = g.cfg.name.clone();
            let msg = Sync {
                kind: "join".into(),
                room,
                host: host.clone(),
                name,
                video: None,
                paused: false,
                pos: 0.0,
                speed: 1.0,
                queue: Vec::new(),
                qpos: None,
            };
            if let Ok(j) = serde_json::to_string(&msg) {
                send_now(&g, &format!("TO {host} {j}"));
            }
            true
        }
    };
    if !ok {
        toast(cx, "Connect to the relay first");
        return;
    }
    toast(cx, "Joining room");
    cx.refresh_windows();
}

pub fn leave_room(cx: &mut App) {
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if let Some(info) = g.room.take() {
            send_now(&g, &format!("LEAVE {}", info.id));
        }
        g.host = false;
        g.ready = false;
        g.last_sent = None;
        g.status = if g.out.is_some() { Status::Online } else { Status::Offline };
    }
    cx.refresh_windows();
}

pub fn invite(cx: &mut App, friend_id: String) {
    let err = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        if !g.host {
            Some("Only the room host can invite")
        } else if g.room.is_none() {
            Some("Create a room first")
        } else {
            let info = g.room.as_ref().unwrap();
            let msg = Sync {
                kind: "invite".into(),
                room: info.id.clone(),
                host: g.cfg.me.clone(),
                name: g.cfg.name.clone(),
                video: None,
                paused: false,
                pos: 0.0,
                speed: 1.0,
                queue: Vec::new(),
                qpos: None,
            };
            if let Ok(j) = serde_json::to_string(&msg) {
                send_now(&g, &format!("TO {friend_id} {j}"));
                None
            } else {
                Some("Could not build the invite")
            }
        }
    };
    match err {
        Some(m) => toast(cx, m),
        None => toast(cx, "Invite sent"),
    }
}

pub fn accept_invite(cx: &mut App) {
    let inv = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        g.invite.clone()
    };
    if let Some(inv) = inv {
        join_room(cx, inv.room, inv.host);
    }
}

pub fn decline_invite(cx: &mut App) {
    let r = cx.default_global::<Room>();
    r.inner.lock().unwrap().invite = None;
    cx.refresh_windows();
}

fn send_now(g: &Inner, line: &str) {
    if let Some(tx) = &g.out {
        let _ = tx.send(line.to_string());
    }
}

// ── network ────────────────────────────────────────────────────────────────

fn net_loop(relay: String, token: String, me: String, _name: String, tx: futures::channel::mpsc::UnboundedSender<Event>) {
    let addr = match relay.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => {
            let _ = tx.unbounded_send(Event::Down(format!("bad relay address: {relay}")));
            return;
        }
    };
    let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(8)) {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(format!("could not reach relay: {e}")));
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    let mut wr = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(e.to_string()));
            return;
        }
    };
    let ctl = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(e.to_string()));
            return;
        }
    };
    if write_line(&mut wr, &format!("HELLO {token} {me}")).is_err() {
        let _ = tx.unbounded_send(Event::Down("relay closed during hello".into()));
        return;
    }
    let (line_tx, line_rx) = mpsc::channel::<String>();
    let _ = tx.unbounded_send(Event::Connected(line_tx, ctl));
    std::thread::spawn(move || writer(&mut wr, line_rx));

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        if tx.unbounded_send(Event::Line(line)).is_err() {
            return;
        }
    }
    let _ = tx.unbounded_send(Event::Down("relay connection closed".into()));
}

fn writer(stream: &mut TcpStream, rx: Receiver<String>) {
    loop {
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(line) => {
                if write_line(stream, &line).is_err() {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if write_line(stream, "PING").is_err() {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn write_line(stream: &mut TcpStream, line: &str) -> std::io::Result<()> {
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

// ── relay events ───────────────────────────────────────────────────────────

fn handle_event(cx: &mut App, ev: Event) {
    match ev {
        Event::Connected(out, ctl) => {
            let r = cx.default_global::<Room>();
            let mut g = r.inner.lock().unwrap();
            g.out = Some(out);
            g.ctl = Some(ctl);
        }
        Event::Down(reason) => {
            let was = {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                let was = g.status;
                g.out = None;
                g.ctl = None;
                g.status = Status::Offline;
                g.room = None;
                g.host = false;
                g.ready = false;
                g.last_sent = None;
                g.error = Some(reason);
                was
            };
            if was != Status::Offline {
                toast(cx, "Relay disconnected");
            }
            cx.refresh_windows();
        }
        Event::Line(line) => {
            let mut dirty_ui = false;
            if let Some(rest) = line.strip_prefix("WELCOME ") {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                if g.status == Status::Connecting {
                    g.status = Status::Online;
                }
                let _ = rest;
                dirty_ui = true;
            } else if let Some(rest) = line.strip_prefix("ONLINE ") {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                g.online = rest.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
                dirty_ui = true;
            } else if let Some(rest) = line.strip_prefix("ERROR ") {
                let r = cx.default_global::<Room>();
                r.inner.lock().unwrap().error = Some(rest.to_string());
                dirty_ui = true;
            } else if let Some(rest) = line.strip_prefix("PONG") {
                let _ = rest;
            } else if let Some(rest) = line.strip_prefix("FROM ").or_else(|| line.strip_prefix("ROOM ")) {
                if let Some((_from, json)) = rest.split_once(' ') {
                    if let Ok(msg) = serde_json::from_str::<Sync>(json) {
                        on_message(cx, msg);
                        dirty_ui = true;
                    }
                }
            }
            if dirty_ui {
                cx.refresh_windows();
            }
        }
    }
}

fn on_message(cx: &mut App, msg: Sync) {
    match msg.kind.as_str() {
        "invite" => {
            let inv = Invite { room: msg.room.clone(), host: msg.host.clone(), name: msg.name.clone() };
            let (room, host, who) = (inv.room.clone(), inv.host.clone(), inv.name.clone());
            {
                let r = cx.default_global::<Room>();
                r.inner.lock().unwrap().invite = Some(inv);
            }
            let accept: std::sync::Arc<dyn Fn(&mut Window, &mut App)> = std::sync::Arc::new(move |_w, cx| {
                join_room(cx, room.clone(), host.clone());
            });
            let decline: std::sync::Arc<dyn Fn(&mut Window, &mut App)> =
                std::sync::Arc::new(move |_w, cx| decline_invite(cx));
            toast_with_actions(
                cx,
                format!("{who} invited you to a watch party"),
                vec![("Join".into(), accept), ("Later".into(), decline)],
            );
        }
        "state" => {
            let ok = {
                let r = cx.default_global::<Room>();
                let g = r.inner.lock().unwrap();
                !g.host && g.status == Status::InRoom && g.room.as_ref().is_some_and(|r| r.id == msg.room)
            };
            if ok {
                apply_sync(cx, &msg, false);
            }
        }
        "intent" => {
            let ok = {
                let r = cx.default_global::<Room>();
                let g = r.inner.lock().unwrap();
                g.host && g.status == Status::InRoom && g.room.as_ref().is_some_and(|r| r.id == msg.room)
            };
            if ok {
                apply_sync(cx, &msg, true);
            }
        }
        "join" => {
            let is_host = { cx.default_global::<Room>().inner.lock().unwrap().host };
            if is_host {
                // the next heartbeat (or forced send) brings the guest up to date
                cx.default_global::<Room>().inner.lock().unwrap().dirty = true;
            }
        }
        _ => {}
    }
}

// ── player sync ────────────────────────────────────────────────────────────

fn open_watch(cx: &mut App, id: &str, pos: f64) {
    let pip = crate::ui::pip::handle(cx);
    let Some(win) = cx.windows().into_iter().find(|w| Some(*w) != pip) else { return };
    let route = crate::ui::app::Route::Watch {
        id: id.to_string(),
        playlist: None,
        start: (pos > 1.0).then_some(pos),
    };
    let _ = win.update(cx, |_, window, cx| crate::ui::app::navigate(route, window, cx));
}

/// Applies a remote state (guest) or intent (host). `from_guest` means the host
/// is applying a guest's intent, so the result must be broadcast again.
fn apply_sync(cx: &mut App, s: &Sync, from_guest: bool) {
    let pe = controller::player(cx);
    let current = pe.read(cx).video.as_ref().map(|v| v.id.clone());
    let mut video_changed = false;
    if let Some(v) = &s.video {
        if current.as_deref() != Some(v.id.as_str()) {
            open_watch(cx, &v.id, s.pos);
            video_changed = true;
        }
    }
    let pe = controller::player(cx);
    let loading = pe.read(cx).loading;
    pe.update(cx, |p, _| {
        if p.paused != s.paused {
            p.set_paused(s.paused);
        }
        if s.speed > 0.0 && (p.speed - s.speed).abs() > 0.001 {
            p.set_speed(s.speed);
        }
        if !s.queue.is_empty() && (p.queue != s.queue || p.queue_pos != s.qpos) {
            p.queue = s.queue.clone();
            p.queue_pos = s.qpos;
        }
        if !video_changed && !loading && (p.time - s.pos).abs() > 1.0 {
            p.seek_to(s.pos);
        }
    });

    if from_guest {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.obs = Some((s.pos, Instant::now(), s.speed, s.paused));
        g.dirty = true;
    } else {
        // Remember what the player really has, not the (possibly partial)
        // heartbeat, so omitted fields are not mistaken for a change.
        let actual = {
            let pe = controller::player(cx);
            let p = pe.read(cx);
            Sync {
                kind: "intent".into(),
                room: String::new(),
                host: String::new(),
                name: String::new(),
                video: p.video.clone(),
                paused: p.paused,
                pos: p.time,
                speed: p.speed,
                queue: p.queue.clone(),
                qpos: p.queue_pos,
            }
        };
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.obs = Some((actual.pos, Instant::now(), actual.speed, actual.paused));
        g.last_sent = Some(actual);
        g.ready = true;
    }
}

fn tick(cx: &mut App) {
    let now = Instant::now();
    // Presence: ask the relay who of our friends is online, every few seconds.
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.status != Status::Offline && now.duration_since(g.who) >= Duration::from_secs(5) {
            let ids: Vec<String> = g.cfg.friends.iter().map(|f| f.id.clone()).collect();
            if !ids.is_empty() {
                send_now(&g, &format!("WHO {}", ids.join(",")));
            }
            g.who = now;
        }
    }
    let (host, room_id, host_id, last_sent, obs, beat, ready, dirty) = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        if g.status != Status::InRoom {
            return;
        }
        let Some(info) = &g.room else { return };
        (g.host, info.id.clone(), info.host.clone(), g.last_sent.clone(), g.obs, g.beat, g.ready, g.dirty)
    };
    if !host && !ready {
        return;
    }

    let pe = controller::player(cx);
    let (video, paused, time, speed, queue, qpos, loading) = {
        let p = pe.read(cx);
        (p.video.clone(), p.paused, p.time, p.speed, p.queue.clone(), p.queue_pos, p.loading)
    };

    // A big jump away from the expected position is a seek, not playback.
    let mut seek = false;
    if let Some((t0, at, sp, pa)) = obs {
        let expected = if pa { t0 } else { t0 + at.elapsed().as_secs_f64() * sp };
        if !loading && (time - expected).abs() > 1.5 {
            seek = true;
        }
    }
    let new_obs = Some((time, now, speed, paused));

    let changed = match &last_sent {
        None => true,
        Some(prev) => {
            prev.paused != paused
                || (prev.speed - speed).abs() > 0.001
                || prev.video.as_ref().map(|v| &v.id) != video.as_ref().map(|v| &v.id)
                || prev.queue != queue
                || prev.qpos != qpos
                || seek
        }
    };
    let beat_due = host && now.duration_since(beat) >= Duration::from_secs(1);
    if !changed && !dirty && !beat_due {
        let r = cx.default_global::<Room>();
        r.inner.lock().unwrap().obs = new_obs;
        return;
    }

    let name = {
        let r = cx.default_global::<Room>();
        r.inner.lock().unwrap().cfg.name.clone()
    };
    let full = Sync {
        kind: if host { "state" } else { "intent" }.to_string(),
        room: room_id.clone(),
        host: host_id.clone(),
        name: name.clone(),
        video,
        paused,
        pos: time,
        speed,
        queue,
        qpos,
    };
    // A plain heartbeat carries only position/state. Video and queue are sent
    // when they change, so the wire stays small even with a long queue.
    let out = if changed || dirty {
        full.clone()
    } else {
        Sync {
            kind: if host { "state" } else { "intent" }.to_string(),
            room: room_id.clone(),
            host: host_id.clone(),
            name,
            video: None,
            paused,
            pos: time,
            speed,
            queue: Vec::new(),
            qpos: None,
        }
    };

    if let Ok(j) = serde_json::to_string(&out) {
        let line = if host { format!("ROOM {room_id} {j}") } else { format!("TO {host_id} {j}") };
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        send_now(&g, &line);
        g.last_sent = Some(full);
        g.beat = now;
        g.dirty = false;
        g.obs = new_obs;
    }
}

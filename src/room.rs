//! Watch-party rooms: relay client, saved friends, invites and player sync.
//!
//! One peer hosts a room and is the authority. Guests send intents to the host;
//! the host applies them locally and broadcasts state to the room. The relay
//! (tools/relay) only routes these tiny control lines; video never passes
//! through it. Sync drives the embedded player directly, so no mpv IPC is used.
use crate::api::types::VideoItem;
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
#[derive(Clone, Default, Serialize, Deserialize)]
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
    /// Names currently in the room, from the host.
    #[serde(default)]
    pub members: Vec<String>,
    /// A state message carries the whole queue (even an empty one). Heartbeats
    /// leave it false and omit the queue to stay small.
    #[serde(default)]
    pub full: bool,
    /// For "activity": "like", "dislike" or "share".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: String,
    /// A "share" sent to every friend instead of one.
    #[serde(default)]
    pub general: bool,
    /// Unix seconds, for ordering the feed.
    #[serde(default)]
    pub at: u64,
}

fn one() -> f64 {
    1.0
}

/// One friend's like, dislike or shared video.
#[derive(Clone)]
pub struct Activity {
    pub from_id: String,
    pub from_name: String,
    pub action: String,
    pub video: Option<VideoItem>,
    pub general: bool,
    pub at: u64,
}

/// A room a friend currently hosts, advertised so others can join uninvited.
#[derive(Clone)]
pub struct RoomAd {
    pub room: String,
    pub host: String,
    pub host_name: String,
    pub at: u64,
}

/// A guest the host has heard from recently.
#[derive(Clone)]
struct Member {
    name: String,
    seen: Instant,
}

/// One emoji/GIF reaction, kept briefly so the overlay can animate it.
#[derive(Clone)]
pub struct Reaction {
    pub seq: u64,
    pub from: String,
    pub action: String,
    pub at: Instant,
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
    /// Bumped on every connect/disconnect so events from an old socket are
    /// ignored and cannot overwrite a newer connection's status.
    conn: u64,
    last_sent: Option<Sync>,
    /// last (time, at, speed, paused) seen from the player, to tell a seek from
    /// normal playback
    obs: Option<(f64, Instant, f64, bool)>,
    beat: Instant,
    who: Instant,
    ready: bool,
    dirty: bool,
    feed: Vec<Activity>,
    unread: u32,
    rooms: Vec<RoomAd>,
    rooms_beat: Instant,
    /// Guests heard from recently (host) or the last roster (guest).
    members: Vec<Member>,
    /// Last "still here" ping a guest sent the host.
    here: Instant,
    reactions: Vec<Reaction>,
    next_reaction: u64,
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
            conn: 0,
            last_sent: None,
            obs: None,
            beat: Instant::now(),
            who: Instant::now(),
            ready: false,
            dirty: false,
            feed: Vec::new(),
            unread: 0,
            rooms: Vec::new(),
            rooms_beat: Instant::now(),
            members: Vec::new(),
            here: Instant::now(),
            reactions: Vec::new(),
            next_reaction: 0,
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
    pub feed: Vec<Activity>,
    pub unread: u32,
    pub rooms: Vec<RoomAd>,
    pub members: Vec<String>,
    /// Who hosts (a name, not an id).
    pub host_name: String,
    pub reactions: Vec<Reaction>,
}

pub fn snapshot(cx: &mut App) -> Snapshot {
    let r = cx.default_global::<Room>();
    let mut g = r.inner.lock().unwrap();
    let now = now_secs();
    g.rooms.retain(|r| now.saturating_sub(r.at) < 60);
    Snapshot {
        cfg: g.cfg.clone(),
        status: g.status,
        error: g.error.clone(),
        online: g.online.clone(),
        room: g.room.clone(),
        host: g.host,
        invite: g.invite.clone(),
        feed: g.feed.clone(),
        unread: g.unread,
        rooms: g.rooms.clone(),
        members: roster(&g),
        host_name: host_name(&g),
        reactions: g.reactions.clone(),
    }
}

/// The host's display name: our own name while hosting, otherwise the saved
/// friend's name for the room's host id.
fn host_name(g: &Inner) -> String {
    if g.host {
        return g.cfg.name.clone();
    }
    g.room
        .as_ref()
        .and_then(|r| g.cfg.friends.iter().find(|f| f.id == r.host).map(|f| f.name.clone()))
        .unwrap_or_else(|| "Host".into())
}

/// The people in the room: you first, then the guests we heard from.
fn roster(g: &Inner) -> Vec<String> {
    let mut names = vec![g.cfg.name.clone()];
    for m in &g.members {
        if !names.iter().any(|n| n == &m.name) {
            names.push(m.name.clone());
        }
    }
    names
}

/// Records a guest and returns true when it is a new name.
fn remember(g: &mut Inner, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    if let Some(m) = g.members.iter_mut().find(|m| m.name == name) {
        m.seen = Instant::now();
        false
    } else {
        g.members.push(Member { name: name.to_string(), seen: Instant::now() });
        if g.members.len() > 32 {
            g.members.remove(0);
        }
        true
    }
}

fn forget(g: &mut Inner, name: &str) {
    let name = name.trim();
    g.members.retain(|m| m.name != name);
}

enum Event {
    Connected(u64, Sender<String>, TcpStream),
    Line(u64, String),
    Down(u64, String),
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
    let (conn_gen, old_ctl) = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        // Drop any previous socket before opening a new one.
        g.out = None;
        g.conn = g.conn.wrapping_add(1);
        g.status = Status::Connecting;
        g.error = None;
        (g.conn, g.ctl.take())
    };
    if let Some(s) = old_ctl {
        let _ = s.shutdown(Shutdown::Both);
    }
    cx.refresh_windows();
    let Some(tx) = events() else { return };
    std::thread::spawn(move || net_loop(relay, token, me, name, conn_gen, tx));
}

pub fn disconnect(window: &mut Window, cx: &mut App) {
    // Closing the relay also ends the party: close the room tab (which leaves
    // the room) so no empty room tab is left behind.
    crate::ui::app::close_room(window, cx);
    let ctl = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        g.out = None;
        g.conn = g.conn.wrapping_add(1);
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

pub fn create_room(window: &mut Window, cx: &mut App) {
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
            g.members.clear();
            g.dirty = true;
            send_now(&g, &format!("JOIN {id}"));
            let msg = Sync {
                kind: "room".into(),
                action: "open".into(),
                room: id.clone(),
                host: g.cfg.me.clone(),
                name: g.cfg.name.clone(),
                at: now_secs(),
                speed: 1.0,
                ..Default::default()
            };
            send_to_friends(&g, &msg);
            g.rooms_beat = Instant::now();
            true
        }
    };
    if !ok {
        toast(cx, "Connect to the relay first");
        return;
    }
    crate::ui::app::open_room(window, cx);
    toast(cx, "Room created. Invite a friend.");
    cx.refresh_windows();
}

pub fn join_room(window: &mut Window, cx: &mut App, room: String, host: String) {
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
            g.members.clear();
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
                ..Default::default()
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
    crate::ui::app::open_room(window, cx);
    toast(cx, "Joining room");
    cx.refresh_windows();
}

pub fn leave_room(cx: &mut App) {
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.host {
            if let Some(info) = &g.room {
                let msg = Sync {
                    kind: "room".into(),
                    action: "closed".into(),
                    room: info.id.clone(),
                    host: g.cfg.me.clone(),
                    name: g.cfg.name.clone(),
                    at: now_secs(),
                    speed: 1.0,
                    ..Default::default()
                };
                send_to_friends(&g, &msg);
            }
        } else if let Some(info) = &g.room {
            // Tell the host so the roster drops us right away.
            let leave = Sync {
                kind: "leave".into(),
                room: info.id.clone(),
                host: info.host.clone(),
                name: g.cfg.name.clone(),
                ..Default::default()
            };
            if let Ok(j) = serde_json::to_string(&leave) {
                send_now(&g, &format!("TO {} {j}", info.host));
            }
        }
        if let Some(info) = g.room.take() {
            send_now(&g, &format!("LEAVE {}", info.id));
        }
        g.host = false;
        g.ready = false;
        g.last_sent = None;
        g.members.clear();
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
                ..Default::default()
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

pub fn accept_invite(window: &mut Window, cx: &mut App) {
    let inv = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        g.invite.clone()
    };
    if let Some(inv) = inv {
        join_room(window, cx, inv.room, inv.host);
    }
}

pub fn decline_invite(cx: &mut App) {
    let r = cx.default_global::<Room>();
    r.inner.lock().unwrap().invite = None;
    cx.refresh_windows();
}

/// True while a room tab is open (the shared queue and player exist).
pub fn in_room(cx: &App) -> bool {
    crate::ui::app::room_player(cx).is_some()
}

/// Adds a video to the room queue. The first video starts the room playback.
pub fn enqueue(v: VideoItem, cx: &mut App) {
    let Some(pe) = crate::ui::app::room_player(cx) else {
        toast(cx, "Open a room first");
        return;
    };
    pe.update(cx, |p, cx| {
        if p.video.is_none() {
            p.queue = vec![v.clone()];
            p.queue_pos = Some(0);
            p.play(v, None, cx);
        } else {
            if p.queue.is_empty() {
                if let Some(cur) = p.video.clone() {
                    p.queue.push(cur);
                    p.queue_pos = Some(0);
                }
            }
            p.queue.push(v);
        }
        cx.notify();
    });
}

/// Adds a video to the room queue and plays it right away.
pub fn play_now(v: VideoItem, cx: &mut App) {
    let Some(pe) = crate::ui::app::room_player(cx) else {
        toast(cx, "Open a room first");
        return;
    };
    pe.update(cx, |p, cx| {
        if let Some(pos) = p.queue.iter().position(|x| x.id == v.id) {
            p.queue_pos = Some(pos);
        } else {
            p.queue.push(v.clone());
            p.queue_pos = Some(p.queue.len() - 1);
        }
        p.play(v, None, cx);
        cx.notify();
    });
}

/// Plays a row of the room queue in the room player.
pub fn play_index(i: usize, cx: &mut App) {
    if let Some(pe) = crate::ui::app::room_player(cx) {
        pe.update(cx, |p, cx| {
            if let Some(v) = p.queue.get(i).cloned() {
                p.queue_pos = Some(i);
                p.play(v, None, cx);
                cx.notify();
            }
        });
    }
}

/// The new index of the playing row after moving `from` to `to`.
fn moved_pos(cur: usize, from: usize, to: usize) -> usize {
    if cur == from {
        to
    } else if from < cur && to >= cur {
        cur - 1
    } else if from > cur && to <= cur {
        cur + 1
    } else {
        cur
    }
}

/// Moves a queue row to another position, keeping the current row current.
pub fn move_queue(from: usize, to: usize, cx: &mut App) {
    let Some(pe) = crate::ui::app::room_player(cx) else { return };
    pe.update(cx, |p, cx| {
        if from == to || from >= p.queue.len() || to >= p.queue.len() {
            return;
        }
        let item = p.queue.remove(from);
        p.queue.insert(to, item);
        if let Some(cur) = p.queue_pos {
            p.queue_pos = Some(moved_pos(cur, from, to));
        }
        cx.notify();
    });
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn send_to_friends(g: &Inner, msg: &Sync) -> bool {
    let Ok(j) = serde_json::to_string(msg) else { return false };
    let mut any = false;
    for f in &g.cfg.friends {
        if g.out.is_some() {
            send_now(g, &format!("TO {} {j}", f.id));
            any = true;
        }
    }
    any
}

/// Records what this install shared or rated, for the Social "Your activity" card.
fn record_own(g: &mut Inner, action: &str, video: Option<VideoItem>, general: bool) {
    g.feed.push(Activity {
        from_id: g.cfg.me.clone(),
        from_name: "You".into(),
        action: action.to_string(),
        video,
        general,
        at: now_secs(),
    });
    if g.feed.len() > 100 {
        g.feed.remove(0);
    }
}

/// Shares a video with every saved friend (general activity).
pub fn share_all(video: VideoItem, cx: &mut App) {
    let sent = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        let msg = Sync {
            kind: "share".into(),
            name: g.cfg.name.clone(),
            video: Some(video.clone()),
            general: true,
            at: now_secs(),
            speed: 1.0,
            ..Default::default()
        };
        let sent = send_to_friends(&g, &msg);
        if sent {
            record_own(&mut g, "share", Some(video), true);
        }
        sent
    };
    if sent {
        toast(cx, "Shared with friends");
    } else {
        toast(cx, "Connect to the relay and add a friend first");
    }
}

/// Shares a video with one friend (shows up in their notifications).
pub fn share_to(friend_id: String, video: VideoItem, cx: &mut App) {
    let sent = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        let msg = Sync {
            kind: "share".into(),
            name: g.cfg.name.clone(),
            video: Some(video.clone()),
            general: false,
            at: now_secs(),
            speed: 1.0,
            ..Default::default()
        };
        let sent = match serde_json::to_string(&msg) {
            Ok(j) if g.out.is_some() => {
                send_now(&g, &format!("TO {friend_id} {j}"));
                true
            }
            _ => false,
        };
        if sent {
            record_own(&mut g, "share", Some(video), false);
        }
        sent
    };
    if sent {
        toast(cx, "Video shared");
    } else {
        toast(cx, "Connect to the relay first");
    }
}

/// Sends a like/dislike to friends when the matching sharing setting is on.
/// The action is recorded locally either way, so Social can show your activity.
pub fn activity(action: &str, video: VideoItem, cx: &mut App) {
    let s = store::settings_snapshot();
    let on = match action {
        "like" => s.share_likes,
        "dislike" => s.share_dislikes,
        _ => false,
    };
    let r = cx.default_global::<Room>();
    let mut g = r.inner.lock().unwrap();
    if on {
        let msg = Sync {
            kind: "activity".into(),
            name: g.cfg.name.clone(),
            action: action.to_string(),
            video: Some(video.clone()),
            general: true,
            at: now_secs(),
            speed: 1.0,
            ..Default::default()
        };
        send_to_friends(&g, &msg);
    }
    record_own(&mut g, action, Some(video), on);
}

fn record_reaction(g: &mut Inner, from: String, action: &str) {
    g.next_reaction = g.next_reaction.wrapping_add(1);
    let seq = g.next_reaction;
    g.reactions.push(Reaction { seq, from, action: action.to_string(), at: Instant::now() });
    if g.reactions.len() > 40 {
        g.reactions.remove(0);
    }
    g.reactions.retain(|r| r.at.elapsed() < Duration::from_secs(8));
}

/// Sends an emoji/GIF reaction to the room. The host is the authority: a guest
/// sends it to the host, who rebroadcasts it to everyone.
pub fn react(action: &str, cx: &mut App) {
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        let Some(info) = g.room.clone() else { return };
        let msg = Sync {
            kind: "reaction".into(),
            room: info.id.clone(),
            host: info.host.clone(),
            name: g.cfg.name.clone(),
            action: action.to_string(),
            at: now_secs(),
            ..Default::default()
        };
        let Ok(j) = serde_json::to_string(&msg) else { return };
        if g.host {
            send_now(&g, &format!("ROOM {} {j}", info.id));
            let name = g.cfg.name.clone();
            record_reaction(&mut g, name, action);
        } else {
            send_now(&g, &format!("TO {} {j}", info.host));
        }
    }
    cx.refresh_windows();
}

/// Unread direct shares, for the top-bar bell.
pub fn unread(cx: &App) -> u32 {
    cx.try_global::<Room>().map(|r| r.inner.lock().unwrap().unread).unwrap_or(0)
}

pub fn mark_read(cx: &mut App) {
    let r = cx.default_global::<Room>();
    r.inner.lock().unwrap().unread = 0;
    cx.refresh_windows();
}

/// The video to share from the Social page: the room video if one is open,
/// otherwise the active tab's video, then any tab's last video.
pub fn current_video(cx: &App) -> Option<VideoItem> {
    if let Some(pe) = crate::ui::app::room_player(cx) {
        if let Some(v) = pe.read(cx).video.clone() {
            return Some(v);
        }
    }
    crate::player::controller::player(cx)
        .read(cx)
        .video
        .clone()
        .or_else(|| crate::ui::app::recent_video(cx))
}

/// Menu items for the top-bar bell: the latest direct shares.
pub fn notification_items(cx: &mut App) -> Vec<crate::ui::app::MenuItem> {
    let feed: Vec<Activity> = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        g.feed.iter().filter(|a| a.action == "share" && !a.general).rev().take(8).cloned().collect()
    };
    if feed.is_empty() {
        return vec![crate::ui::app::MenuItem::new("bell", "No shared videos yet", |_, _| {})];
    }
    feed.into_iter()
        .map(|a| {
            let id = a.video.as_ref().map(|v| v.id.clone()).unwrap_or_default();
            let title = a.video.as_ref().map(|v| if v.title.is_empty() { v.id.clone() } else { v.title.clone() }).unwrap_or_else(|| "a video".into());
            let label = format!("{} shared: {}", a.from_name, title);
            crate::ui::app::MenuItem::new("play", label, move |window, cx| {
                if !id.is_empty() {
                    crate::ui::app::navigate(
                        crate::ui::app::Route::Watch { id: id.clone(), playlist: None, start: None },
                        window,
                        cx,
                    );
                }
            })
        })
        .collect()
}

fn send_now(g: &Inner, line: &str) {
    if let Some(tx) = &g.out {
        let _ = tx.send(line.to_string());
    }
}

// ── network ────────────────────────────────────────────────────────────────

fn net_loop(relay: String, token: String, me: String, _name: String, conn_gen: u64, tx: futures::channel::mpsc::UnboundedSender<Event>) {
    let addr = match relay.to_socket_addrs().ok().and_then(|mut a| a.next()) {
        Some(a) => a,
        None => {
            let _ = tx.unbounded_send(Event::Down(conn_gen, format!("bad relay address: {relay}")));
            return;
        }
    };
    let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(8)) {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(conn_gen, format!("could not reach relay: {e}")));
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    let mut wr = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(conn_gen, e.to_string()));
            return;
        }
    };
    let ctl = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.unbounded_send(Event::Down(conn_gen, e.to_string()));
            return;
        }
    };
    if write_line(&mut wr, &format!("HELLO {token} {me}")).is_err() {
        let _ = tx.unbounded_send(Event::Down(conn_gen, "relay closed during hello".into()));
        return;
    }
    let (line_tx, line_rx) = mpsc::channel::<String>();
    let _ = tx.unbounded_send(Event::Connected(conn_gen, line_tx, ctl));
    std::thread::spawn(move || writer(&mut wr, line_rx));

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        if tx.unbounded_send(Event::Line(conn_gen, line)).is_err() {
            return;
        }
    }
    let _ = tx.unbounded_send(Event::Down(conn_gen, "relay connection closed".into()));
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
    let conn_gen = match &ev {
        Event::Connected(g, _, _) | Event::Line(g, _) | Event::Down(g, _) => *g,
    };
    // Ignore anything left over from a replaced socket.
    if cx.default_global::<Room>().inner.lock().unwrap().conn != conn_gen {
        return;
    }
    match ev {
        Event::Connected(_, out, ctl) => {
            let r = cx.default_global::<Room>();
            let mut g = r.inner.lock().unwrap();
            g.out = Some(out);
            g.ctl = Some(ctl);
        }
        Event::Down(_, reason) => {
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
                g.members.clear();
                g.error = Some(reason);
                was
            };
            if was != Status::Offline {
                toast(cx, "Relay disconnected");
            }
            cx.refresh_windows();
        }
        Event::Line(_, line) => {
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
                if let Some((from, json)) = rest.split_once(' ') {
                    if let Ok(msg) = serde_json::from_str::<Sync>(json) {
                        on_message(cx, from, msg);
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

fn on_message(cx: &mut App, from: &str, msg: Sync) {
    match msg.kind.as_str() {
        "share" => {
            let name = if msg.name.trim().is_empty() { from.chars().take(8).collect::<String>() } else { msg.name.clone() };
            let act = Activity {
                from_id: from.to_string(),
                from_name: name.clone(),
                action: "share".into(),
                video: msg.video.clone(),
                general: msg.general,
                at: msg.at,
            };
            let direct = !msg.general;
            {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                g.feed.push(act);
                if g.feed.len() > 100 {
                    g.feed.remove(0);
                }
                if direct {
                    g.unread = g.unread.saturating_add(1);
                }
            }
            if direct {
                toast(cx, format!("{name} shared a video with you"));
            }
            cx.refresh_windows();
        }
        "activity" => {
            let name = if msg.name.trim().is_empty() { from.chars().take(8).collect::<String>() } else { msg.name.clone() };
            let act = Activity {
                from_id: from.to_string(),
                from_name: name,
                action: if msg.action.is_empty() { "activity".into() } else { msg.action.clone() },
                video: msg.video.clone(),
                general: msg.general,
                at: msg.at,
            };
            {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                g.feed.push(act);
                if g.feed.len() > 100 {
                    g.feed.remove(0);
                }
            }
            cx.refresh_windows();
        }
        "room" => {
            let host = if msg.host.is_empty() { from.to_string() } else { msg.host.clone() };
            let host_name = if msg.name.trim().is_empty() { from.chars().take(8).collect::<String>() } else { msg.name.clone() };
            {
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                g.rooms.retain(|r| r.host != host);
                if msg.action != "closed" && !msg.room.is_empty() {
                    g.rooms.push(RoomAd { room: msg.room.clone(), host, host_name, at: now_secs() });
                }
            }
            cx.refresh_windows();
        }
        "reaction" => {
            let (in_room, host) = {
                let r = cx.default_global::<Room>();
                let g = r.inner.lock().unwrap();
                let in_room = g.room.as_ref().is_some_and(|r| r.id == msg.room);
                (in_room && g.status == Status::InRoom, g.host)
            };
            if in_room {
                let name = guest_name(&msg, from);
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                record_reaction(&mut g, name, &msg.action);
                if host {
                    // A guest's reaction came through TO; fan it out to the room.
                    if let Ok(j) = serde_json::to_string(&msg) {
                        send_now(&g, &format!("ROOM {} {j}", msg.room));
                    }
                }
                drop(g);
                cx.refresh_windows();
            }
        }
        "invite" => {
            let inv = Invite { room: msg.room.clone(), host: msg.host.clone(), name: msg.name.clone() };
            let (room, host, who) = (inv.room.clone(), inv.host.clone(), inv.name.clone());
            {
                let r = cx.default_global::<Room>();
                r.inner.lock().unwrap().invite = Some(inv);
            }
            let accept: std::sync::Arc<dyn Fn(&mut Window, &mut App)> = std::sync::Arc::new(move |w, cx| {
                join_room(w, cx, room.clone(), host.clone());
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
                let r = cx.default_global::<Room>();
                let mut g = r.inner.lock().unwrap();
                // The host owns the roster. Keep only what it sent.
                g.members = msg
                    .members
                    .iter()
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| Member { name: n.clone(), seen: Instant::now() })
                    .collect();
            }
        }
        "intent" => {
            let ok = {
                let r = cx.default_global::<Room>();
                let g = r.inner.lock().unwrap();
                g.host && g.status == Status::InRoom && g.room.as_ref().is_some_and(|r| r.id == msg.room)
            };
            if ok {
                let name = guest_name(&msg, from);
                if msg.action == "leave" {
                    let r = cx.default_global::<Room>();
                    let mut g = r.inner.lock().unwrap();
                    forget(&mut g, &name);
                    g.dirty = true;
                } else {
                    {
                        let r = cx.default_global::<Room>();
                        let mut g = r.inner.lock().unwrap();
                        if remember(&mut g, &name) {
                            g.dirty = true;
                        }
                    }
                    apply_sync(cx, &msg, true);
                }
            }
        }
        "join" => {
            let name = guest_name(&msg, from);
            let r = cx.default_global::<Room>();
            let mut g = r.inner.lock().unwrap();
            if g.host {
                remember(&mut g, &name);
                // the next heartbeat (or forced send) brings the guest up to date
                g.dirty = true;
            }
        }
        "here" => {
            let name = guest_name(&msg, from);
            let r = cx.default_global::<Room>();
            let mut g = r.inner.lock().unwrap();
            if g.host {
                if remember(&mut g, &name) {
                    g.dirty = true;
                }
            }
        }
        "leave" => {
            let name = guest_name(&msg, from);
            let r = cx.default_global::<Room>();
            let mut g = r.inner.lock().unwrap();
            if g.host {
                forget(&mut g, &name);
                g.dirty = true;
            }
        }
        _ => {}
    }
}

/// A guest's display name, falling back to the start of their id.
fn guest_name(msg: &Sync, from: &str) -> String {
    if msg.name.trim().is_empty() {
        from.chars().take(8).collect()
    } else {
        msg.name.trim().to_string()
    }
}

// ── player sync ────────────────────────────────────────────────────────────

/// Applies a remote state (guest) or intent (host) to the room tab's player.
/// `from_guest` means the host is applying a guest's intent, so the result must
/// be broadcast again.
fn apply_sync(cx: &mut App, s: &Sync, from_guest: bool) {
    let Some(pe) = crate::ui::app::room_player(cx) else { return };
    let current = pe.read(cx).video.as_ref().map(|v| v.id.clone());
    let video_changed = s.video.as_ref().is_some_and(|v| current.as_deref() != Some(v.id.as_str()));
    pe.update(cx, |p, cx| {
        if let Some(v) = &s.video {
            if p.video.as_ref().map(|x| x.id.as_str()) != Some(v.id.as_str()) {
                p.play(v.clone(), Some(s.pos), cx);
            }
        }
        if p.paused != s.paused {
            p.set_paused(s.paused, cx);
        }
        if s.speed > 0.0 && (p.speed - s.speed).abs() > 0.001 {
            p.set_speed(s.speed);
        }
        // Only a full state may touch the queue (and it may clear it); a
        // heartbeat omits the queue.
        if (s.full || !s.queue.is_empty()) && (p.queue != s.queue || p.queue_pos != s.qpos) {
            p.queue = s.queue.clone();
            p.queue_pos = s.qpos;
        }
        if !video_changed && !p.loading && (p.time - s.pos).abs() > 1.0 {
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
                ..Default::default()
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
    // Keep the reaction overlay alive while reactions are visible.
    let animate = {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        let before = g.reactions.len();
        g.reactions.retain(|r| r.at.elapsed() < Duration::from_secs(6));
        !g.reactions.is_empty() || g.reactions.len() != before
    };
    if animate {
        cx.refresh_windows();
    }
    // Hosting: advertise the room to friends so they can join without an invite.
    {
        let r = cx.default_global::<Room>();
        let mut g = r.inner.lock().unwrap();
        if g.host && g.status == Status::InRoom && now.duration_since(g.rooms_beat) >= Duration::from_secs(10) {
            if let Some(info) = &g.room {
                let msg = Sync {
                    kind: "room".into(),
                    action: "open".into(),
                    room: info.id.clone(),
                    host: g.cfg.me.clone(),
                    name: g.cfg.name.clone(),
                    at: now_secs(),
                    speed: 1.0,
                    ..Default::default()
                };
                send_to_friends(&g, &msg);
            }
            g.rooms_beat = now;
        }
    }
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
        // Guests tell the host they are still here; the host expires the quiet.
        if g.status == Status::InRoom && !g.host && now.duration_since(g.here) >= Duration::from_secs(5) {
            if let Some(info) = &g.room {
                let msg = Sync {
                    kind: "here".into(),
                    room: info.id.clone(),
                    host: info.host.clone(),
                    name: g.cfg.name.clone(),
                    ..Default::default()
                };
                if let Ok(j) = serde_json::to_string(&msg) {
                    send_now(&g, &format!("TO {} {j}", info.host));
                }
            }
            g.here = now;
        }
        if g.host {
            let before = g.members.len();
            g.members.retain(|m| m.seen.elapsed() < Duration::from_secs(12));
            if g.members.len() != before {
                g.dirty = true;
            }
        }
    }
    let (host, room_id, host_id, last_sent, members, obs, beat, ready, dirty) = {
        let r = cx.default_global::<Room>();
        let g = r.inner.lock().unwrap();
        if g.status != Status::InRoom {
            return;
        }
        let Some(info) = &g.room else { return };
        (g.host, info.id.clone(), info.host.clone(), g.last_sent.clone(), roster(&g), g.obs, g.beat, g.ready, g.dirty)
    };
    if !host && !ready {
        return;
    }

    let Some(pe) = crate::ui::app::room_player(cx) else { return };
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
        members: members.clone(),
        full: true,
        ..Default::default()
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
            members,
            ..Default::default()
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


#[cfg(test)]
mod presence_tests {
    use super::{forget, guest_name, remember, roster, Inner, Sync};

    fn test_inner() -> Inner {
        let mut g = Inner::new();
        g.cfg.name = "Me".into();
        g
    }

    #[test]
    fn roster_puts_you_first_and_never_duplicates() {
        let mut g = test_inner();
        assert_eq!(roster(&g), vec!["Me".to_string()]);
        assert!(remember(&mut g, "Alice"));
        assert_eq!(roster(&g), vec!["Me".to_string(), "Alice".to_string()]);
        // hearing from the same guest again does not add a second row
        assert!(!remember(&mut g, "Alice"));
        assert_eq!(roster(&g), vec!["Me".to_string(), "Alice".to_string()]);
        // a guest using your display name cannot shadow you
        remember(&mut g, "Me");
        assert_eq!(roster(&g), vec!["Me".to_string(), "Alice".to_string()]);
        forget(&mut g, "Alice");
        assert_eq!(roster(&g), vec!["Me".to_string()]);
    }

    #[test]
    fn an_empty_name_falls_back_to_the_id() {
        let msg = Sync { name: String::new(), ..Default::default() };
        assert_eq!(guest_name(&msg, "abcdef0123456789"), "abcdef01");
        let msg = Sync { name: "  Alice ".into(), ..Default::default() };
        assert_eq!(guest_name(&msg, "abcdef0123456789"), "Alice");
    }
}
#[cfg(test)]
mod queue_tests {
    use super::moved_pos;

    #[test]
    fn reordering_keeps_the_playing_row_pointing_at_the_same_video() {
        // moving rows after the current one does not move the current row
        assert_eq!(moved_pos(0, 2, 1), 0);
        // moving the current row follows it
        assert_eq!(moved_pos(1, 1, 3), 3);
        // moving a row from before the current one down past it shifts it down
        assert_eq!(moved_pos(2, 0, 2), 1);
        // moving a row from after the current one up to the top shifts it up
        assert_eq!(moved_pos(0, 2, 0), 1);
        // a move that stays before the current row leaves it alone
        assert_eq!(moved_pos(3, 1, 0), 3);
    }
}
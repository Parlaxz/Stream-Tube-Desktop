//! Stream Tube relay: a tiny, dependency-free message router for watch parties.
//!
//! It never touches video and never parses payloads. It only routes one-line
//! frames between connected apps and fans room messages out to members. Payloads
//! are opaque, so the app owns their format.
//!
//! Frame = `<VERB> <arg> [<rest of line>]`. The last field is passed through
//! untouched, which lets JSON payloads contain spaces. One newline per frame.
//!
//! Build: `rustc --edition 2021 -O relay.rs -o relay.exe`
//! Run:   `relay.exe --port 8930 --token <secret>`
//! Check: `relay.exe --selftest`
//!
//! Client -> relay:  HELLO <token> <id> | TO <id> <json> | DM <id> <json> |
//!                   JOIN <room> | LEAVE <room> | ROOM <room> <json> |
//!                   WHO <id,id,..> | PING
//! Relay -> client:  WELCOME <id> | FROM <id> <json> | ROOM <id> <json> |
//!                   ONLINE <id,id,..> | PONG | ERROR <text>
//!
//! DMs are the one thing the relay stores. `DM` appends `to<TAB>from<TAB>json`
//! to a tab-separated file and forwards it. On HELLO both sides' stored DMs are
//! replayed, so an offline friend still gets the message.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Writer = Arc<Mutex<TcpStream>>;

struct Dm {
    to: String,
    from: String,
    payload: String,
}

struct Hub {
    token: String,
    clients: Mutex<HashMap<String, Writer>>,
    rooms: Mutex<HashMap<String, HashSet<String>>>,
    /// Stored DMs. The file is append-only; the vector is the in-memory copy.
    dms: Mutex<Vec<Dm>>,
    dm_file: Option<PathBuf>,
}

impl Hub {
    fn new(token: String, dm_file: Option<PathBuf>) -> Self {
        let dms = dm_file.as_deref().map(load_dms).unwrap_or_default();
        Self {
            token,
            clients: Mutex::new(HashMap::new()),
            rooms: Mutex::new(HashMap::new()),
            dms: Mutex::new(dms),
            dm_file,
        }
    }

    /// Stores a DM and tries to deliver it now. The recipient gets it from the
    /// replay on its next HELLO if it is offline.
    fn store_dm(&self, to: &str, from: &str, payload: &str) {
        if let Some(path) = &self.dm_file {
            let line = format!("{to}\t{from}\t{payload}\n");
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = f.write_all(line.as_bytes());
            }
        }
        self.dms.lock().unwrap().push(Dm { to: to.to_string(), from: from.to_string(), payload: payload.to_string() });
    }

    /// Sends every stored DM that involves `id` to `id`, oldest first.
    fn replay_dms(&self, id: &str) {
        let dms = self.dms.lock().unwrap();
        for d in dms.iter().filter(|d| d.to == id || d.from == id) {
            self.send(id, &format!("FROM {} {}", d.from, d.payload));
        }
    }

    fn send(&self, id: &str, line: &str) -> bool {
        let w = self.clients.lock().unwrap().get(id).cloned();
        w.map(|w| write_line(&w, line)).unwrap_or(false)
    }

    fn broadcast_room(&self, room: &str, from: &str, line: &str) {
        let targets: Vec<Writer> = {
            let rooms = self.rooms.lock().unwrap();
            let members = rooms.get(room).cloned().unwrap_or_default();
            let clients = self.clients.lock().unwrap();
            members.iter().filter(|m| m.as_str() != from).filter_map(|m| clients.get(m.as_str()).cloned()).collect()
        };
        for t in targets {
            write_line(&t, line);
        }
    }

    fn online(&self, csv: &str) -> String {
        let clients = self.clients.lock().unwrap();
        csv.split(',').filter(|s| !s.is_empty() && clients.contains_key(*s)).collect::<Vec<_>>().join(",")
    }

    /// Drops `id` only while it still points at this connection, so a reconnect
    /// that replaced the old writer is not torn down by the old thread.
    fn remove_if(&self, id: &str, w: &Writer) {
        let mut clients = self.clients.lock().unwrap();
        if clients.get(id).is_some_and(|cur| Arc::ptr_eq(cur, w)) {
            clients.remove(id);
        }
        drop(clients);
        for set in self.rooms.lock().unwrap().values_mut() {
            set.remove(id);
        }
    }
}

fn load_dms(path: &Path) -> Vec<Dm> {
    let Ok(f) = std::fs::File::open(path) else { return Vec::new() };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            let mut it = line.splitn(3, '\t');
            Some(Dm { to: it.next()?.to_string(), from: it.next()?.to_string(), payload: it.next()?.to_string() })
        })
        .collect()
}

fn write_line(w: &Writer, line: &str) -> bool {
    let mut g = w.lock().unwrap();
    g.write_all(line.as_bytes()).is_ok() && g.write_all(b"\n").is_ok()
}

fn serve_conn(hub: Arc<Hub>, stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
    let writer = match stream.try_clone() {
        Ok(w) => Arc::new(Mutex::new(w)),
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    let mut me: Option<String> = None;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if !handle(&hub, &writer, &mut me, line) {
            break;
        }
    }
    if let Some(id) = me {
        hub.remove_if(&id, &writer);
    }
}

fn handle(hub: &Hub, w: &Writer, me: &mut Option<String>, line: &str) -> bool {
    let mut it = line.splitn(3, ' ');
    let verb = it.next().unwrap_or("");
    match verb {
        "HELLO" => {
            let token = it.next().unwrap_or("");
            let id = it.next().unwrap_or("");
            if token != hub.token {
                write_line(w, "ERROR bad token");
                return false;
            }
            if id.is_empty() || id.contains(' ') {
                write_line(w, "ERROR bad id");
                return false;
            }
            hub.clients.lock().unwrap().insert(id.to_string(), w.clone());
            *me = Some(id.to_string());
            write_line(w, &format!("WELCOME {id}"));
            hub.replay_dms(id);
            true
        }
        "TO" => {
            let Some(from) = me.clone() else { return false };
            let to = it.next().unwrap_or("");
            let payload = it.next().unwrap_or("");
            hub.send(to, &format!("FROM {from} {payload}"));
            true
        }
        "DM" => {
            let Some(from) = me.clone() else { return false };
            let to = it.next().unwrap_or("");
            let payload = it.next().unwrap_or("");
            if !to.is_empty() && !payload.is_empty() {
                hub.store_dm(to, &from, payload);
                hub.send(to, &format!("FROM {from} {payload}"));
            }
            true
        }
        "JOIN" => {
            let Some(from) = me.clone() else { return false };
            let room = it.next().unwrap_or("");
            if !room.is_empty() {
                hub.rooms.lock().unwrap().entry(room.to_string()).or_default().insert(from);
            }
            true
        }
        "LEAVE" => {
            let Some(from) = me.clone() else { return false };
            let room = it.next().unwrap_or("");
            if let Some(set) = hub.rooms.lock().unwrap().get_mut(room) {
                set.remove(&from);
            }
            true
        }
        "ROOM" => {
            let Some(from) = me.clone() else { return false };
            let room = it.next().unwrap_or("");
            let payload = it.next().unwrap_or("");
            hub.broadcast_room(room, &from, &format!("ROOM {from} {payload}"));
            true
        }
        "WHO" => {
            let ids = it.next().unwrap_or("");
            write_line(w, &format!("ONLINE {}", hub.online(ids)))
        }
        "PING" => write_line(w, "PONG"),
        _ => write_line(w, "ERROR unknown verb"),
    }
}

fn arg_value(args: &[String], name: &str, default: &str) -> String {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(selftest());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("usage: relay [--port 8930] [--token <secret>] [--bind 0.0.0.0] [--data relay_dms.tsv] [--selftest]");
        return;
    }
    let port: u16 = arg_value(&args, "--port", "8930").parse().unwrap_or(8930);
    let bind = arg_value(&args, "--bind", "0.0.0.0");
    let token = arg_value(&args, "--token", "");
    let data = arg_value(&args, "--data", "relay_dms.tsv");
    if token.is_empty() {
        eprintln!("warning: no --token given, anyone who finds the port can connect");
    }
    let hub = Arc::new(Hub::new(token, Some(PathBuf::from(data))));
    let listener = match TcpListener::bind((bind.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not bind {bind}:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!("streamtube relay listening on {bind}:{port} (token: {})", if hub.token.is_empty() { "none" } else { "set" });
    for stream in listener.incoming().flatten() {
        let hub = hub.clone();
        std::thread::spawn(move || serve_conn(hub, stream));
    }
}

// ── self-check: real sockets, real routing ──────────────────────────────────

fn client(port: u16) -> (TcpStream, BufReader<TcpStream>) {
    let s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(2))).expect("timeout");
    let r = BufReader::new(s.try_clone().expect("clone"));
    (s, r)
}

fn send(s: &TcpStream, line: &str) {
    let mut s = s;
    s.write_all(line.as_bytes()).expect("write");
    s.write_all(b"\n").expect("write");
}

fn expect(r: &mut BufReader<TcpStream>, want: &str) {
    let mut line = String::new();
    r.read_line(&mut line).expect("read");
    let got = line.trim_end();
    assert_eq!(got, want, "relay sent {got:?}, expected {want:?}");
}

fn selftest() -> i32 {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("selftest bind failed: {e}");
            return 1;
        }
    };
    let port = listener.local_addr().unwrap().port();
    let hub = Arc::new(Hub::new("t".to_string(), None));
    {
        let hub = hub.clone();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let h = hub.clone();
                std::thread::spawn(move || serve_conn(h, s));
            }
        });
    }

    let (a, mut ar) = client(port);
    let (b, mut br) = client(port);
    send(&a, "HELLO t alice");
    expect(&mut ar, "WELCOME alice");
    send(&b, "HELLO t bob");
    expect(&mut br, "WELCOME bob");

    send(&a, "TO bob {\"t\":\"invite\",\"room\":\"r1\"}");
    expect(&mut br, "FROM alice {\"t\":\"invite\",\"room\":\"r1\"}");

    send(&a, "JOIN r1");
    send(&a, "PING");
    expect(&mut ar, "PONG");
    send(&b, "JOIN r1");
    send(&b, "PING");
    expect(&mut br, "PONG");

    send(&a, "ROOM r1 {\"t\":\"state\",\"pos\":1.5}");
    expect(&mut br, "ROOM alice {\"t\":\"state\",\"pos\":1.5}");

    send(&b, "WHO alice,bob,carol");
    expect(&mut br, "ONLINE alice,bob");

    let (c, mut cr) = client(port);
    send(&c, "HELLO wrong eve");
    expect(&mut cr, "ERROR bad token");

    // A DM to an offline friend is stored and replayed on their next HELLO.
    send(&a, "DM carol {\"t\":\"dm\",\"dm_id\":\"m1\"}");
    let (d, mut dr) = client(port);
    send(&d, "HELLO t carol");
    expect(&mut dr, "WELCOME carol");
    expect(&mut dr, "FROM alice {\"t\":\"dm\",\"dm_id\":\"m1\"}");

    println!("selftest PASS: hello, unicast, rooms, presence, token check and DM replay");
    0
}

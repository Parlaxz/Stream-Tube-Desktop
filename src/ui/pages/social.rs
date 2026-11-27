//! Social page: a Home hub for rooms and activity, plus connection and friend
//! settings.
use crate::room;
use crate::ui::*;
use gpui_kit::component::input::{Input, InputState};
use std::sync::atomic::{AtomicU8, Ordering};

/// Asks the next Social page build to open on Messages (paper-plane button).
static DM_REQUEST: AtomicU8 = AtomicU8::new(0);

pub fn request_dms() {
    DM_REQUEST.store(1, Ordering::SeqCst);
}

#[derive(Clone, Copy, PartialEq)]
enum SocialTab {
    Home,
    Messages,
    Settings,
}

pub struct SocialPage {
    relay: Entity<InputState>,
    token: Entity<InputState>,
    name: Entity<InputState>,
    add_id: Entity<InputState>,
    add_name: Entity<InputState>,
    dm_input: Entity<InputState>,
    dm_to: Option<String>,
    show_codes: bool,
    tab: SocialTab,
}

impl SocialPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let snap = room::snapshot(cx);
        let relay = cx.new(|cx| {
            let mut i = InputState::new(window, cx).placeholder("nucbox-ip:8930");
            i.set_value(snap.cfg.relay.clone(), window, cx);
            i
        });
        let token = cx.new(|cx| {
            let mut i = InputState::new(window, cx).placeholder("relay token");
            i.set_value(snap.cfg.token.clone(), window, cx);
            i
        });
        let name = cx.new(|cx| {
            let mut i = InputState::new(window, cx).placeholder("your display name");
            i.set_value(snap.cfg.name.clone(), window, cx);
            i
        });
        let add_id = cx.new(|cx| InputState::new(window, cx).placeholder("friend code"));
        let add_name = cx.new(|cx| InputState::new(window, cx).placeholder("name"));
        let dm_input = cx.new(|cx| InputState::new(window, cx).placeholder("Message"));
        let tab = if DM_REQUEST.swap(0, Ordering::SeqCst) == 1 { SocialTab::Messages } else { SocialTab::Home };
        let dm_to = (tab == SocialTab::Messages).then(|| snap.cfg.friends.first().map(|f| f.id.clone())).flatten();
        Self { relay, token, name, add_id, add_name, dm_input, dm_to, show_codes: false, tab }
    }
}

fn button(id: impl Into<ElementId>, label: &str, p: Pal, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .px_4()
        .h_9()
        .rounded_lg()
        .border_1()
        .border_color(p.border)
        .bg(if primary { p.accent } else { p.chip })
        .text_color(if primary { white() } else { p.text })
        .cursor_pointer()
        .hover(|s| s.opacity(0.88))
        .child(label.to_string())
}

fn field(label: &str, input: &Entity<InputState>, w: f32, p: Pal) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(p.text2).child(label.to_string()))
        .child(
            div()
                .w(px(w))
                .px_3()
                .py_1()
                .rounded_lg()
                .border_1()
                .border_color(p.border)
                .bg(p.bg)
                .child(Input::new(input).appearance(false)),
        )
}

fn card(p: Pal) -> Div {
    div().flex().flex_col().gap_3().rounded_xl().bg(p.surface).p_5()
}

fn dot(online: bool, p: Pal) -> Div {
    div()
        .flex_none()
        .size(px(8.))
        .rounded_full()
        .bg(if online { Hsla::from(rgb(0x2ba640)) } else { p.text3 })
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn activity_row(a: &room::Activity, p: Pal, cx: &mut App) -> AnyElement {
    let verb = match a.action.as_str() {
        "like" => "liked",
        "dislike" => "disliked",
        "share" => "shared",
        _ => "posted",
    };
    let mut col = div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_lg()
        .bg(p.chip)
        .child(div().text_xs().text_color(p.text2).child(format!("{} {}", a.from_name, verb)));
    if let Some(v) = &a.video {
        col = col.child(crate::ui::cards::compact_row(v, false, None, cx));
    }
    col.into_any_element()
}

fn tab_pill(id: &'static str, label: &str, active: bool, enabled: bool, p: Pal) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .px_4()
        .h(px(32.))
        .rounded_full()
        .bg(if active { p.accent } else { p.chip })
        .text_sm()
        .when(active, |d| d.font_weight(FontWeight::MEDIUM))
        .text_color(if active { white() } else { p.text })
        .cursor_pointer()
        .hover(|s| s.opacity(0.88))
        .when(!enabled, |d| d.opacity(0.35))
        .child(label.to_string())
}

impl Render for SocialPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let snap = room::snapshot(cx);
        let connected = snap.status != room::Status::Offline;
        let in_room = snap.room.is_some();
        let (status, status_color): (&str, Hsla) = match snap.status {
            room::Status::Offline => ("Offline", p.text3),
            room::Status::Connecting => ("Connecting…", p.text2),
            room::Status::Online => ("Connected to relay", rgb(0x2ba640).into()),
            room::Status::InRoom => ("In a room", rgb(0x2ba640).into()),
        };

        // ── settings: connection form ──
        let mut conn = card(p)
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Connection"))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_3()
                    .child(field("Relay (host:port)", &self.relay, 220., p))
                    .child(field("Token", &self.token, 180., p))
                    .child(field("Your name", &self.name, 140., p))
                    .child(
                        button("room-connect", if snap.status == room::Status::Offline { "Connect" } else { "Reconnect" }, p, true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let relay = this.relay.read(cx).value().trim().to_string();
                                let token = this.token.read(cx).value().trim().to_string();
                                let name = this.name.read(cx).value().trim().to_string();
                                // A reconnect ends the old session cleanly: one
                                // socket, and a room cannot outlive its relay.
                                if room::snapshot(cx).status != room::Status::Offline {
                                    room::disconnect(window, cx);
                                }
                                room::save_settings(relay, token, name, cx);
                                room::connect(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(dot(connected, p))
                    .child(div().text_sm().text_color(status_color).child(status.to_string()))
                    .when(connected, |d| {
                        d.child(button("room-off", "Disconnect", p, false).on_click(cx.listener(|_, _, w, cx| {
                            room::disconnect(w, cx);
                            cx.notify();
                        })))
                    }),
            )
            .when_some(snap.error.clone(), |d, e| d.child(div().text_xs().text_color(rgb(0xff4e45)).child(e)));

        // my friend code, hidden until asked for
        let mut code_row = div()
            .flex()
            .items_center()
            .gap_2()
            .pt_2()
            .child(button("social-code-toggle", if self.show_codes { "Hide friend code" } else { "Show friend code" }, p, false).on_click(
                cx.listener(|this, _, _, cx| {
                    this.show_codes = !this.show_codes;
                    cx.notify();
                }),
            ));
        if self.show_codes {
            let code = snap.cfg.me.clone();
            code_row = code_row
                .child(div().flex_1().min_w_0().text_xs().text_color(p.text).truncate().child(code.clone()))
                .child(button("room-copy", "Copy", p, false).on_click(cx.listener(|_, _, _w, cx| {
                    let code = room::snapshot(cx).cfg.me;
                    crate::ui::copy_to_clipboard(code, cx);
                })));
        }
        conn = conn.child(code_row);

        // ── settings: friends ──
        let mut friends = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Friends"));
        friends = friends.child(
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(field("Friend code", &self.add_id, 260., p))
                .child(field("Name", &self.add_name, 140., p))
                .child(button("room-add", "Add", p, false).on_click(cx.listener(|this, _, window, cx| {
                    let id = this.add_id.read(cx).value().trim().to_string();
                    let name = this.add_name.read(cx).value().trim().to_string();
                    room::add_friend(id, name, cx);
                    this.add_id.update(cx, |i, cx| i.set_value("", window, cx));
                    this.add_name.update(cx, |i, cx| i.set_value("", window, cx));
                    cx.notify();
                }))),
        );
        if snap.cfg.friends.is_empty() {
            friends = friends.child(div().text_sm().text_color(p.text3).child("Add a friend by their code."));
        }
        for f in snap.cfg.friends.clone() {
            let online = snap.online.iter().any(|x| x == &f.id);
            let can_invite = snap.host && snap.room.is_some() && online;
            let fid = f.id.clone();
            let short: String = f.id.chars().take(12).collect();
            friends = friends.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .h(px(34.))
                    .child(dot(online, p))
                    .child(div().text_sm().text_color(p.text).truncate().child(f.name.clone()))
                    .child(div().flex_1().min_w_0().text_xs().text_color(p.text3).truncate().child(format!("{short}…")))
                    .when(can_invite, |d| {
                        d.child(button(SharedString::from(format!("room-inv-{}", f.id)), "Invite", p, false).on_click(
                            cx.listener(move |_, _, _w, cx| {
                                room::invite(cx, fid.clone());
                                cx.notify();
                            }),
                        ))
                    })
                    .child(button(SharedString::from(format!("room-del-{}", f.id)), "Remove", p, false).on_click(
                        cx.listener({
                            let fid = f.id.clone();
                            move |_, _, _w, cx| {
                                room::remove_friend(fid.clone(), cx);
                                cx.notify();
                            }
                        }),
                    )),
            );
        }

        // ── home: room status / creation ──
        let mut room_card = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Room"));
        if let Some(info) = &snap.room {
            let role = if snap.host { "You are hosting" } else { "You are a guest" };
            room_card = room_card
                .child(div().flex().items_center().gap_2()
                    .child(div().text_sm().text_color(p.text).child(format!("Room {}", info.id)))
                    .child(div().text_xs().text_color(p.text2).child(format!("{role} · {} in room", snap.members.len().max(1)))))
                .child(div().flex().items_center().gap_2()
                    .child(button("room-open-lobby", "Open room", p, true).on_click(cx.listener(|_, _, w, cx| {
                        crate::ui::app::open_room(w, cx);
                    })))
                    .child(button("room-leave", "Leave room", p, false).on_click(cx.listener(|_, _, w, cx| {
                        crate::ui::app::close_room(w, cx);
                        cx.notify();
                    }))));
        } else {
            room_card = room_card.child(
                div().flex().items_center().gap_3()
                    .child(button("room-create", "Create room", p, true).on_click(cx.listener(|_, _, w, cx| {
                        room::create_room(w, cx);
                        cx.notify();
                    })))
                    .child(div().text_sm().text_color(p.text2).child("Then invite a friend.")),
            );
        }
        if let Some(inv) = &snap.invite {
            room_card = room_card.child(
                div().flex().items_center().gap_3().pt_2()
                    .child(div().text_sm().text_color(p.text).child(format!("{} invited you", inv.name)))
                    .child(button("room-accept", "Join", p, true).on_click(cx.listener(|_, _, w, cx| {
                        room::accept_invite(w, cx);
                        cx.notify();
                    })))
                    .child(button("room-decline", "Later", p, false).on_click(cx.listener(|_, _, _w, cx| {
                        room::decline_invite(cx);
                        cx.notify();
                    }))),
            );
        }

        // ── home: friends' rooms ──
        let mut rooms = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Friends' rooms"));
        if snap.rooms.is_empty() {
            rooms = rooms.child(div().text_sm().text_color(p.text3).child("No friend is hosting a room right now. Create one to start."));
        }
        for r in snap.rooms.clone() {
            let (room_id, host) = (r.room.clone(), r.host.clone());
            rooms = rooms.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .h(px(34.))
                    .child(dot(true, p))
                    .child(div().flex_1().min_w_0().text_sm().text_color(p.text).truncate().child(format!("{}'s room", r.host_name)))
                    .child(button(SharedString::from(format!("social-join-{}", r.host)), "Join", p, true).on_click(
                        cx.listener(move |_, _, w, cx| {
                            room::join_room(w, cx, room_id.clone(), host.clone());
                            cx.notify();
                        }),
                    )),
            );
        }

        // ── home: feeds ──
        let me = snap.cfg.me.clone();
        let direct: Vec<room::Activity> = snap
            .feed
            .iter()
            .rev()
            .filter(|a| a.from_id != me && a.action == "share" && !a.general)
            .take(12)
            .cloned()
            .collect();
        let general: Vec<room::Activity> = snap
            .feed
            .iter()
            .rev()
            .filter(|a| a.from_id != me && (a.action != "share" || a.general))
            .take(20)
            .cloned()
            .collect();
        let mine: Vec<room::Activity> = snap.feed.iter().rev().filter(|a| a.from_id == me).take(20).cloned().collect();

        let mut shared = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Shared with you"));
        if direct.is_empty() {
            shared = shared.child(div().text_sm().text_color(p.text3).child("Nothing yet. Friends can send you a video from the Share button under any video."));
        } else {
            for a in &direct {
                shared = shared.child(activity_row(a, p, cx));
            }
        }

        let mut acts = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Friends' activity"));
        if general.is_empty() {
            acts = acts.child(
                div()
                    .text_sm()
                    .text_color(p.text3)
                    .child("Likes, dislikes and videos friends share will show here. Friends choose what they share in Settings → Social."),
            );
        } else {
            for a in &general {
                acts = acts.child(activity_row(a, p, cx));
            }
        }

        let mut own = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Your activity"));
        if mine.is_empty() {
            own = own.child(div().text_sm().text_color(p.text3).child("Videos you share, like or dislike show here."));
        } else {
            for a in &mine {
                own = own.child(activity_row(a, p, cx));
            }
        }

        // ── compose ──
        let mut content = div().flex().flex_col().gap_4();
        match self.tab {
            SocialTab::Home => {
                let header = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(p.text).child("Social"))
                            .child(div().text_xs().text_color(p.text2).child(if connected {
                                format!("{} as {}", snap.cfg.relay, snap.cfg.name)
                            } else {
                                "Not connected. Open Settings to connect.".to_string()
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(dot(connected, p))
                            .child(div().text_sm().text_color(status_color).child(status.to_string()))
                            .child(button("social-go-settings", "Settings", p, false).on_click(cx.listener(|this, _, _, cx| {
                                this.tab = SocialTab::Settings;
                                cx.notify();
                            }))),
                    );
                let left = div().flex().flex_col().gap_4().child(room_card).child(rooms).child(own);
                let right = div().flex().flex_col().gap_4().child(shared).child(acts);
                let ui = *cx.global::<crate::ui::app::UiState>();
                let wide = (ui.viewport.width - px(280.)) > px(900.);
                let feeds: AnyElement = if wide {
                    div()
                        .flex()
                        .gap_4()
                        .items_start()
                        .child(div().flex_1().min_w_0().child(left))
                        .child(div().flex_1().min_w_0().child(right))
                        .into_any_element()
                } else {
                    div().flex().flex_col().gap_4().child(left).child(right).into_any_element()
                };
                content = content.child(header).child(feeds);
            }
            SocialTab::Messages => {
                let me = snap.cfg.me.clone();
                let friends = snap.cfg.friends.clone();
                let dms = snap.dms.clone();
                let online = snap.online.clone();
                if self.dm_to.is_none() {
                    self.dm_to = friends.first().map(|f| f.id.clone());
                }
                // reactions grouped under the message they belong to
                let mut reactions: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
                for d in dms.iter().filter(|d| !d.react_to.is_empty()) {
                    reactions.entry(d.react_to.clone()).or_default().push(d.emoji.clone());
                }

                let mut list = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Messages"));
                if friends.is_empty() {
                    list = list.child(div().text_sm().text_color(p.text3).child("Add a friend in Settings to send videos."));
                }
                let mut ordered = friends.clone();
                ordered.sort_by_key(|f| room::conversation(&dms, &me, &f.id).last().map(|d| d.at).unwrap_or(0));
                ordered.reverse();
                for f in &ordered {
                    let fid = f.id.clone();
                    let conv = room::conversation(&dms, &me, &fid);
                    let preview = conv
                        .last()
                        .map(|d| {
                            if !d.react_to.is_empty() {
                                format!("Reacted {}", d.emoji)
                            } else if d.video.is_some() && d.text.is_empty() {
                                "Shared a video".into()
                            } else {
                                d.text.clone()
                            }
                        })
                        .unwrap_or_default();
                    let selected = self.dm_to.as_deref() == Some(&fid);
                    let is_online = online.iter().any(|x| x == &fid);
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("dm-thread-{fid}")))
                            .flex()
                            .items_center()
                            .gap_3()
                            .p_2()
                            .rounded_lg()
                            .cursor_pointer()
                            .when(selected, |d| d.bg(p.chip))
                            .hover(move |s| s.bg(p.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dm_to = Some(fid.clone());
                                room::mark_dms_read(cx);
                                cx.notify();
                            }))
                            .child(dot(is_online, p))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .flex_1()
                                    .child(div().text_sm().text_color(p.text).truncate().child(f.name.clone()))
                                    .child(div().text_xs().text_color(p.text3).truncate().child(preview.clone())),
                            ),
                    );
                }

                let mut thread = card(p);
                if let Some(fid) = self.dm_to.clone() {
                    let fname = friends
                        .iter()
                        .find(|f| f.id == fid)
                        .map(|f| f.name.clone())
                        .unwrap_or_else(|| fid.chars().take(8).collect());
                    thread = thread.child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(fname));
                    let conv = room::conversation(&dms, &me, &fid);
                    if conv.is_empty() {
                        thread = thread.child(div().text_sm().text_color(p.text3).child("No messages yet."));
                    }
                    for d in conv.iter().filter(|d| d.react_to.is_empty()) {
                        let mine = d.from == me;
                        let mut bubble =
                            div().flex().flex_col().gap_1().max_w(px(520.)).p_3().rounded_xl().bg(if mine { p.accent.opacity(0.16) } else { p.chip });
                        if let Some(v) = &d.video {
                            let (vid, start) = (v.id.clone(), d.pos);
                            let title = if v.title.is_empty() { v.id.clone() } else { v.title.clone() };
                            bubble = bubble.child(
                                div()
                                    .id(SharedString::from(format!("dm-video-{}", d.id)))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |_, _, window, cx| {
                                        crate::ui::app::navigate(
                                            crate::ui::app::Route::Watch { id: vid.clone(), playlist: None, start: Some(start) },
                                            window,
                                            cx,
                                        );
                                    }))
                                    .child(img(SharedString::from(v.thumbnail.clone())).w(px(120.)).h(px(68.)).rounded_md().object_fit(ObjectFit::Cover))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .min_w_0()
                                            .child(div().text_sm().text_color(p.text).line_clamp(2).child(title))
                                            .child(div().text_xs().text_color(p.accent).child(format!("▶ {}", clock(start)))),
                                    ),
                            );
                        }
                        if !d.text.is_empty() {
                            bubble = bubble.child(div().text_sm().text_color(p.text).child(d.text.clone()));
                        }
                        if let Some(emojis) = reactions.get(&d.id) {
                            let mut row = div().flex().flex_wrap().gap_1();
                            for e in emojis {
                                row = row.child(div().text_xs().px_2().py_0p5().rounded_full().bg(p.bg).child(e.clone()));
                            }
                            bubble = bubble.child(row);
                        }
                        let target = d.id.clone();
                        let mut reacts = div().flex().gap_1();
                        for e in ["👍", "❤️", "😂", "🔥"] {
                            let t = target.clone();
                            let em = e.to_string();
                            reacts = reacts.child(
                                div()
                                    .id(SharedString::from(format!("dm-react-{}-{e}", target)))
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(move |s| s.opacity(0.7))
                                    .on_click(cx.listener(move |_, _, _, cx| room::react_dm(t.clone(), em.clone(), cx)))
                                    .child(e),
                            );
                        }
                        bubble = bubble.child(reacts);
                        thread = thread.child(div().flex().w_full().when(mine, |d| d.justify_end()).child(bubble));
                    }
                    let to = fid.clone();
                    thread = thread.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_2()
                            .child(
                                div()
                                    .flex_1()
                                    .px_3()
                                    .py_1()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(p.border)
                                    .bg(p.bg)
                                    .child(Input::new(&self.dm_input).appearance(false)),
                            )
                            .child(icon_button("dm-send", "send", cx).on_click(cx.listener(move |this, _, window, cx| {
                                let text = this.dm_input.read(cx).value().trim().to_string();
                                if text.is_empty() {
                                    return;
                                }
                                room::send_dm(to.clone(), text, None, 0.0, cx);
                                this.dm_input.update(cx, |i, cx| i.set_value("", window, cx));
                                cx.notify();
                            }))),
                    );
                } else {
                    thread = thread.child(div().text_sm().text_color(p.text3).child("Select a friend."));
                }

                let ui = *cx.global::<crate::ui::app::UiState>();
                let wide = (ui.viewport.width - px(280.)) > px(900.);
                let body: AnyElement = if wide {
                    div()
                        .flex()
                        .gap_4()
                        .items_start()
                        .child(div().w(px(260.)).flex_none().child(list))
                        .child(div().flex_1().min_w_0().child(thread))
                        .into_any_element()
                } else {
                    div().flex().flex_col().gap_4().child(list).child(thread).into_any_element()
                };
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(p.text).child("Messages"))
                        .child(body),
                );
            }
            SocialTab::Settings => {
                content = content.child(conn).child(friends);
            }
        }

        let tab_home_active = self.tab == SocialTab::Home;
        let tab_messages_active = self.tab == SocialTab::Messages;
        let tab_settings_active = self.tab == SocialTab::Settings;
        let bar = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                tab_pill("social-tab-home", "Home", tab_home_active, true, p)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tab = SocialTab::Home;
                        cx.notify();
                    })),
            )
            .child(
                tab_pill("social-tab-messages", "Messages", tab_messages_active, true, p).on_click(cx.listener(|this, _, _, cx| {
                    this.tab = SocialTab::Messages;
                    if this.dm_to.is_none() {
                        this.dm_to = room::snapshot(cx).cfg.friends.first().map(|f| f.id.clone());
                    }
                    room::mark_dms_read(cx);
                    cx.notify();
                })),
            )
            .when(in_room, |d| {
                d.child(
                    tab_pill("social-tab-room", "Room", false, true, p)
                        .on_click(cx.listener(|_, _, w, cx| crate::ui::app::open_room(w, cx))),
                )
            })
            .when(!in_room, |d| d.child(tab_pill("social-tab-room", "Room", false, false, p)))
            .child(
                tab_pill("social-tab-settings", "Settings", tab_settings_active, true, p)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tab = SocialTab::Settings;
                        cx.notify();
                    })),
            );

        div()
            .id("social-page")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .p_6()
                            .w_full()
                            .max_w(px(1180.))
                            .child(bar)
                            .child(content),
                    ),
            )
    }
}

//! Social page: a Home hub for rooms and activity, plus connection and friend
//! settings.
use crate::room;
use crate::ui::*;
use gpui_kit::component::input::{Input, InputState};

#[derive(Clone, Copy, PartialEq)]
enum SocialTab {
    Home,
    Settings,
}

pub struct SocialPage {
    relay: Entity<InputState>,
    token: Entity<InputState>,
    name: Entity<InputState>,
    add_id: Entity<InputState>,
    add_name: Entity<InputState>,
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
        Self { relay, token, name, add_id, add_name, show_codes: false, tab: SocialTab::Home }
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
            SocialTab::Settings => {
                content = content.child(conn).child(friends);
            }
        }

        let tab_home_active = self.tab == SocialTab::Home;
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

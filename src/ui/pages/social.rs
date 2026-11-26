//! Watch-party page: relay connection, friend list, room and invites.
use crate::room;
use crate::ui::*;
use gpui_kit::component::input::{Input, InputState};

pub struct SocialPage {
    relay: Entity<InputState>,
    token: Entity<InputState>,
    name: Entity<InputState>,
    add_id: Entity<InputState>,
    add_name: Entity<InputState>,
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
        Self { relay, token, name, add_id, add_name }
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

impl Render for SocialPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let snap = room::snapshot(cx);
        let (status, status_color): (&str, Hsla) = match snap.status {
            room::Status::Offline => ("Offline", p.text3),
            room::Status::Connecting => ("Connecting…", p.text2),
            room::Status::Online => ("Connected to relay", rgb(0x2ba640).into()),
            room::Status::InRoom => ("In a room", rgb(0x2ba640).into()),
        };

        // connection
        let mut conn = card(p)
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Social"))
            .child(div().text_sm().text_color(p.text2).child("One peer hosts; everyone follows. Small relay only — video never passes through it."))
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
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let relay = this.relay.read(cx).value().trim().to_string();
                                let token = this.token.read(cx).value().trim().to_string();
                                let name = this.name.read(cx).value().trim().to_string();
                                room::save_settings(relay, token, name, cx);
                                room::connect(cx);
                                cx.notify();
                            })),
                    )
                    .when(snap.status != room::Status::Offline, |d| {
                        d.child(button("room-off", "Disconnect", p, false).on_click(cx.listener(|_, _, _w, cx| {
                            room::disconnect(cx);
                            cx.notify();
                        })))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(dot(snap.status != room::Status::Offline, p))
                    .child(div().text_sm().text_color(status_color).child(status.to_string()))
                    .when_some(snap.error.clone(), |d, e| d.child(div().text_xs().text_color(rgb(0xff4e45)).child(e))),
            );

        // my friend code
        let code = snap.cfg.me.clone();
        conn = conn.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .pt_2()
                .child(div().text_xs().text_color(p.text2).child("Your friend code (share it so friends can add you)"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().text_xs().text_color(p.text).truncate().child(code.clone()))
                        .child(button("room-copy", "Copy", p, false).on_click(cx.listener(|_, _, _w, cx| {
                            let code = room::snapshot(cx).cfg.me;
                            crate::ui::copy_to_clipboard(code, cx);
                        }))),
                ),
        );

        // friends
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

        // room
        let mut room_card = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Room"));
        if let Some(info) = &snap.room {
            let role = if snap.host { "You are hosting" } else { "You are a guest" };
            room_card = room_card
                .child(div().flex().items_center().gap_2()
                    .child(div().text_sm().text_color(p.text).child(format!("Room {}", info.id)))
                    .child(div().text_xs().text_color(p.text2).child(role)))
                .child(button("room-leave", "Leave room", p, false).on_click(cx.listener(|_, _, _w, cx| {
                    room::leave_room(cx);
                    cx.notify();
                })));
        } else {
            room_card = room_card.child(
                div().flex().items_center().gap_3()
                    .child(button("room-create", "Create room", p, true).on_click(cx.listener(|_, _, _w, cx| {
                        room::create_room(cx);
                        cx.notify();
                    })))
                    .child(div().text_sm().text_color(p.text2).child("Then invite an online friend above.")),
            );
        }
        if let Some(inv) = &snap.invite {
            room_card = room_card.child(
                div().flex().items_center().gap_3().pt_2()
                    .child(div().text_sm().text_color(p.text).child(format!("{} invited you", inv.name)))
                    .child(button("room-accept", "Join", p, true).on_click(cx.listener(|_, _, _w, cx| {
                        room::accept_invite(cx);
                        cx.notify();
                    })))
                    .child(button("room-decline", "Later", p, false).on_click(cx.listener(|_, _, _w, cx| {
                        room::decline_invite(cx);
                        cx.notify();
                    }))),
            );
        }

        div()
            .id("social-page")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_6()
                    .max_w(px(760.))
                    .child(conn)
                    .child(friends)
                    .child(room_card),
            )
    }
}

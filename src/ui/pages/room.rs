//! A room tab: the shared player, the live room queue and the room settings.
//! Sync targets only this tab's player, so videos played in other tabs are
//! never shared.
use crate::api::innertube;
use crate::player::controller::{player, Player};
use crate::room;
use crate::ui::cards::compact_row_with;
use crate::ui::widgets::nested_scroll;
use crate::ui::*;
use gpui_kit::component::input::{Input, InputState};

pub struct RoomPage {
    link: Entity<InputState>,
    queue_open: bool,
    settings_open: bool,
    _player_sub: Subscription,
}

impl RoomPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let link = cx.new(|cx| InputState::new(window, cx).placeholder("Paste a YouTube video link"));
        // The queue and playback state change from the player and from room
        // sync, so the page follows the room player.
        let pl = crate::ui::app::room_player(cx).unwrap_or_else(|| player(cx));
        let sub = cx.observe(&pl, |_, _, cx| cx.notify());
        Self { link, queue_open: true, settings_open: false, _player_sub: sub }
    }

    fn submit(&mut self, play: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.link.read(cx).value().trim().to_string();
        let Some(innertube::ResolvedUrl::Video(id)) = innertube::parse_youtube_url(&text) else {
            toast(cx, "Paste a YouTube video link");
            return;
        };
        let v = crate::api::types::VideoItem {
            id: id.clone(),
            thumbnail: innertube::default_thumb(&id),
            ..Default::default()
        };
        if play {
            room::play_now(v, cx);
        } else {
            room::enqueue(v, cx);
        }
        self.link.update(cx, |i, cx| i.set_value("", window, cx));
        cx.notify();
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

fn card(p: Pal) -> Div {
    div().flex().flex_col().gap_3().rounded_xl().bg(p.surface).p_4()
}

impl RoomPage {
    /// The room queue: the same compact rows as the single-player queue.
    fn queue_card(&self, pe: &Entity<Player>, queue: &[crate::api::types::VideoItem], qpos: Option<usize>, p: Pal, cx: &mut Context<Self>) -> Div {
        let mut list = div().id("room-queue-list").flex().flex_col().gap_1();
        if queue.is_empty() {
            list = list.child(div().p_2().text_sm().text_color(p.text3).child("Nothing queued yet. Paste a link below or use Add to room on any video."));
        }
        for (i, v) in queue.iter().enumerate() {
            let extra = if qpos == Some(i) {
                Vec::new()
            } else {
                let plr = pe.clone();
                vec![crate::ui::app::MenuItem::new("room-q-remove", "Remove from queue", move |_, cx| {
                    plr.update(cx, |p, cx| {
                        if i < p.queue.len() {
                            p.queue.remove(i);
                            if let Some(cur) = p.queue_pos.filter(|c| *c > i) {
                                p.queue_pos = Some(cur - 1);
                            }
                            cx.notify();
                        }
                    });
                })]
            };
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().w_5().text_xs().text_color(p.text2).text_center().child(if qpos == Some(i) { "▶".to_string() } else { (i + 1).to_string() }))
                    .child(compact_row_with(
                        v,
                        qpos == Some(i),
                        Some(std::sync::Arc::new(move |_, cx: &mut App| room::play_index(i, cx))),
                        extra,
                        cx,
                    )),
            );
        }
        let shuffle = pe.clone();
        let clear = pe.clone();
        let close = cx.listener(|this, _, _, cx| {
            this.queue_open = false;
            cx.notify();
        });
        div()
            .flex()
            .flex_col()
            .rounded_xl()
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .rounded_t_xl()
                    .bg(p.surface)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_base().font_weight(FontWeight::BOLD).text_color(p.text).child("Queue"))
                            .child(div().text_xs().text_color(p.text2).child(format!("{} / {}", qpos.map(|p| p + 1).unwrap_or(0), queue.len()))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                icon_button("room-q-shuffle", "shuffle", cx).on_click(move |_, _, cx| {
                                    shuffle.update(cx, |p, cx| {
                                        use rand::seq::SliceRandom;
                                        let cur = p.queue_pos.and_then(|i| p.queue.get(i).cloned());
                                        p.queue.shuffle(&mut rand::rng());
                                        if let Some(c) = cur {
                                            p.queue_pos = p.queue.iter().position(|v| v.id == c.id);
                                        }
                                        cx.notify();
                                    });
                                }),
                            )
                            .child(
                                icon_button("room-q-clear", "x", cx).on_click(move |_, _, cx| {
                                    clear.update(cx, |p, cx| {
                                        p.queue.clear();
                                        p.queue_pos = None;
                                        cx.notify();
                                    });
                                }),
                            )
                            .child(icon_button("room-q-close", "chevron-down", cx).on_click(close)),
                    ),
            )
            .child(nested_scroll(
                "room-queue-wrap",
                div().id("room-queue-scroll").max_h(px(360.)).overflow_y_scroll().p_1().child(list),
            ))
    }

    fn settings_card(&self, snap: &room::Snapshot, p: Pal, cx: &mut Context<Self>) -> Div {
        let mut c = card(p).child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("Room settings"));
        if let Some(info) = &snap.room {
            let role = if snap.host { "You are hosting" } else { "You are a guest" };
            let id = info.id.clone();
            c = c.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().text_color(p.text).child(format!("Room {}", info.id)))
                    .child(div().text_xs().text_color(p.text2).child(role))
                    .child(button("room-copy-code", "Copy code", p, false).on_click(cx.listener(move |_, _, _, cx| {
                        crate::ui::copy_to_clipboard(id.clone(), cx);
                    }))),
            );
        }
        if snap.host {
            let online: Vec<room::Friend> = snap.cfg.friends.iter().filter(|f| snap.online.contains(&f.id)).cloned().collect();
            if online.is_empty() {
                c = c.child(div().text_sm().text_color(p.text3).child("No friends online. Add friends in Social to invite them."));
            } else {
                let mut row = div().flex().flex_wrap().items_center().gap_2();
                for f in online {
                    let fid = f.id.clone();
                    row = row.child(
                        button(SharedString::from(format!("room-inv-{}", f.id)), &format!("Invite {}", f.name), p, false).on_click(
                            cx.listener(move |_, _, _, cx| {
                                room::invite(cx, fid.clone());
                            }),
                        ),
                    );
                }
                c = c.child(row);
            }
        }
        c.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(button("room-goto-social", "Open Social", p, false).on_click(cx.listener(|_, _, _, cx| {
                    crate::ui::app::open_social(cx);
                })))
                .child(button("room-settings-leave", "Leave room", p, false).on_click(cx.listener(|_, _, _, cx| {
                    crate::ui::app::close_room(cx);
                }))),
        )
    }
}

impl Render for RoomPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let snap = room::snapshot(cx);
        let pe = crate::ui::app::room_player(cx).unwrap_or_else(|| player(cx));
        let (queue, qpos, current) = {
            let r = pe.read(cx);
            (r.queue.clone(), r.queue_pos, r.video.clone())
        };

        // No room: a small empty state, not a dead player page.
        let Some(info) = snap.room.clone() else {
            return div()
                .id("room-tab")
                .size_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_4()
                        .p_6()
                        .w_full()
                        .max_w(px(680.))
                        .child(
                            card(p)
                                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child("No room is open"))
                                .child(div().text_sm().text_color(p.text2).child("Create a room to watch together, or join a friend's room from Social.")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(button("room-create", "Create room", p, true).on_click(cx.listener(|_, _, _, cx| {
                                    room::create_room(cx);
                                })))
                                .child(button("room-open-social", "Open Social", p, false).on_click(cx.listener(|_, _, _, cx| {
                                    crate::ui::app::open_social(cx);
                                }))),
                        ),
                );
        };

        let ui = *cx.global::<crate::ui::app::UiState>();
        let side = if ui.sidebar_open { px(232.) } else { px(88.) };
        let avail = (ui.viewport.width - side - px(48.)).max(px(480.));
        let player_w = avail.min(px(1280.));
        let player_h = player_w * (9. / 16.);
        let two_col = player_w > px(900.);

        let role = if snap.host { "You are hosting" } else { "You are a guest" };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .w(player_w)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(p.text).child(format!("Room {}", info.id)))
                    .child(div().text_xs().text_color(p.text2).child(role)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button("room-queue-toggle", &format!("Queue ({})", queue.len()), p, self.queue_open).on_click(cx.listener(|this, _, _, cx| {
                            this.queue_open = !this.queue_open;
                            cx.notify();
                        })),
                    )
                    .child(button("room-settings-toggle", "Room settings", p, self.settings_open).on_click(cx.listener(|this, _, _, cx| {
                        this.settings_open = !this.settings_open;
                        cx.notify();
                    })))
                    .child(button("room-leave", "Leave", p, false).on_click(cx.listener(|_, _, _, cx| {
                        crate::ui::app::close_room(cx);
                    }))),
            );

        let player_box = div()
            .w(player_w)
            .h(player_h)
            .flex_none()
            .relative()
            .rounded_xl()
            .overflow_hidden()
            .child(crate::ui::slot::player_slot(cx));

        let now_playing = if let Some(v) = &current {
            let title = if v.title.is_empty() { v.id.clone() } else { v.title.clone() };
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(crate::ui::widgets::selectable_text(format!("room-vt-{}", v.id), title)))
                .child(div().text_xs().text_color(p.text2).child(v.channel_name.clone()))
        } else {
            div().text_sm().text_color(p.text2).child(if snap.host { "Nothing is playing yet." } else { "Nothing is playing yet." })
        };

        let add_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_3()
                    .py_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(p.border)
                    .bg(p.bg)
                    .child(Input::new(&self.link).appearance(false)),
            )
            .child(button("room-add-queue", "Add to queue", p, false).on_click(cx.listener(|this, _, window, cx| {
                this.submit(false, window, cx);
            })))
            .child(button("room-add-now", "Play now", p, true).on_click(cx.listener(|this, _, window, cx| {
                this.submit(true, window, cx);
            })));

        let settings = self.settings_open.then(|| self.settings_card(&snap, p, cx));
        let left = div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_w_0()
            .child(now_playing)
            .child(add_row)
            .children(settings);

        let queue_panel = self.queue_open.then(|| self.queue_card(&pe, &queue, qpos, p, cx));
        let queue_col = queue_panel.map(|q| {
            div()
                .flex()
                .flex_col()
                .w(if two_col { px(380.) } else { player_w })
                .flex_none()
                .child(q)
        });

        let below = div()
            .flex()
            .gap_4()
            .w(player_w)
            .when(!two_col, |d| d.flex_col())
            .child(left)
            .children(queue_col);

        div()
            .id("room-tab")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .p_6()
                    .w_full()
                    .child(header)
                    .child(player_box)
                    .child(below),
            )
    }
}

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
        // sync, so the page follows the room player. This page is built while
        // the AppView is being updated, so do not read the AppView here: the
        // room tab is the active tab, and `player` already points at its player.
        let pl = player(cx);
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

/// A queue row being dragged to another position.
struct QueueDrag {
    from: usize,
}

/// Ghost shown under the pointer while a queue row is dragged.
struct QueueDragView {
    label: SharedString,
    offset: Point<Pixels>,
}

impl Render for QueueDragView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        div()
            .flex()
            .items_center()
            .gap_2()
            .h(px(32.))
            .px_3()
            .w(px(240.))
            .ml(self.offset.x - px(120.))
            .mt(self.offset.y - px(16.))
            .rounded_lg()
            .bg(p.surface2)
            .shadow_md()
            .opacity(0.92)
            .child(icon("list-video").size_4().text_color(p.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(p.text)
                    .child(self.label.clone()),
            )
    }
}

impl RoomPage {
    /// The room queue: the same compact rows as the single-player queue.
    fn queue_card(&self, pe: &Entity<Player>, queue: &[crate::api::types::VideoItem], qpos: Option<usize>, p: Pal, cx: &mut Context<Self>) -> Div {
        let mut list = div().id("room-queue-list").flex().flex_col().gap_1();
        if queue.is_empty() {
            list = list.child(div().p_2().text_sm().text_color(p.text3).child("Nothing queued yet. Paste a link below or use Add to room on any video."));
        }
        let next_at = qpos.map(|q| q + 1).unwrap_or(0).min(queue.len().saturating_sub(1));
        for (i, v) in queue.iter().enumerate() {
            let extra = if qpos == Some(i) {
                Vec::new()
            } else {
                let mut items = vec![crate::ui::app::MenuItem::new("play", "Play now", move |_, cx| room::play_index(i, cx))];
                if next_at != i {
                    items.push(crate::ui::app::MenuItem::new("list-plus", "Play next", move |_, cx| room::move_queue(i, next_at, cx)));
                }
                if i != 0 {
                    items.push(crate::ui::app::MenuItem::new("chevron-up", "Move to top", move |_, cx| room::move_queue(i, 0, cx)));
                }
                let plr = pe.clone();
                items.push(crate::ui::app::MenuItem::new("list-x", "Remove from queue", move |_, cx| {
                    plr.update(cx, |p, cx| {
                        if i < p.queue.len() {
                            p.queue.remove(i);
                            if let Some(cur) = p.queue_pos.filter(|c| *c > i) {
                                p.queue_pos = Some(cur - 1);
                            }
                            cx.notify();
                        }
                    });
                }));
                items
            };
            let label: SharedString = if v.title.is_empty() { v.id.clone() } else { v.title.clone() }.into();
            let row = div()
                .id(("room-qrow", i))
                .flex()
                .items_center()
                .gap_1()
                .child(div().id(("room-qgrip", i)).flex_none().cursor_grab().child(icon("menu").size_3().text_color(p.text3)))
                .child(div().w_5().text_xs().text_color(p.text2).text_center().child(if qpos == Some(i) { "▶".to_string() } else { (i + 1).to_string() }))
                .child(compact_row_with(
                    v,
                    qpos == Some(i),
                    Some(std::sync::Arc::new(move |_, cx: &mut App| room::play_index(i, cx))),
                    extra,
                    cx,
                ))
                .on_drag(QueueDrag { from: i }, move |_d, offset, _, cx| {
                    let label = label.clone();
                    cx.new(move |_| QueueDragView { label, offset })
                })
                .on_drop::<QueueDrag>(cx.listener(move |_, drag: &QueueDrag, _, cx| {
                    room::move_queue(drag.from, i, cx);
                    cx.notify();
                }));
            list = list.child(row);
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
                .child(button("room-goto-social", "Open Social", p, false).on_click(cx.listener(|_, _, w, cx| {
                    crate::ui::app::open_social(w, cx);
                })))
                .child(button("room-settings-leave", "Leave room", p, false).on_click(cx.listener(|_, _, w, cx| {
                    crate::ui::app::close_room(w, cx);
                }))),
        )
    }
}

impl Render for RoomPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = pal(cx);
        let snap = room::snapshot(cx);
        // The room tab is only rendered while it is active, so the active player
        // is the room player. Reading the AppView here is unsafe during a tab
        // switch (gpui panics on a read while the entity is being updated).
        let pe = player(cx);
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
                                .child(button("room-create", "Create room", p, true).on_click(cx.listener(|_, _, w, cx| {
                                    room::create_room(w, cx);
                                })))
                                .child(button("room-open-social", "Open Social", p, false).on_click(cx.listener(|_, _, w, cx| {
                                    crate::ui::app::open_social(w, cx);
                                }))),
                        ),
                )
                .into_any_element();
        };

        let ui = *cx.global::<crate::ui::app::UiState>();
        if ui.fullscreen {
            // Same as the watch page: the player is the whole window.
            return div()
                .id("room-fullscreen")
                .size_full()
                .bg(black())
                .child(crate::ui::slot::player_slot(cx))
                .into_any_element();
        }

        let theater = crate::store::settings_snapshot().theater_mode;
        let vp = ui.viewport;
        let avail = vp.width - px(48.);
        let primary_w = avail.min(px(1280.));
        let two_col = avail > px(1000.);
        let player_w = if theater { vp.width } else { primary_w };
        let player_h = if theater {
            (vp.width * (9. / 16.)).min(vp.height - px(176.))
        } else {
            primary_w * (9. / 16.)
        };

        let role = if snap.host { "You are hosting" } else { "You are a guest" };
        let count = snap.members.len().max(1);
        let who = snap
            .members
            .iter()
            .map(|n| if n == &snap.cfg.name { "You".to_string() } else { n.clone() })
            .collect::<Vec<_>>()
            .join(", ");
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xl().font_weight(FontWeight::BOLD).text_color(p.text).child(format!("Room {}", info.id)))
                    .child(div().text_xs().text_color(p.text2).child(if who.is_empty() {
                        format!("{role} · {count} in room")
                    } else {
                        format!("{role} · {count} in room: {who}")
                    })),
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
                    .child(button("room-leave", "Leave", p, false).on_click(cx.listener(|_, _, w, cx| {
                        crate::ui::app::close_room(w, cx);
                    }))),
            );

        let player_box = div()
            .w(player_w)
            .h(player_h)
            .flex_none()
            .relative()
            .when(!theater, |d| d.rounded_xl().overflow_hidden())
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
            div().text_sm().text_color(p.text2).child("Nothing is playing yet.")
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
            .child(button("room-paste", "Paste", p, false).on_click(cx.listener(|this, _, window, cx| {
                let text = cx.read_from_clipboard().and_then(|i| i.text()).unwrap_or_default();
                let text = text.trim().to_string();
                if text.is_empty() {
                    toast(cx, "Clipboard is empty");
                    return;
                }
                this.link.update(cx, |i, cx| i.set_value(text, window, cx));
                cx.notify();
            })))
            .child(button("room-add-queue", "Add to queue", p, false).on_click(cx.listener(|this, _, window, cx| {
                this.submit(false, window, cx);
            })))
            .child(button("room-add-now", "Play now", p, true).on_click(cx.listener(|this, _, window, cx| {
                this.submit(true, window, cx);
            })));

        // Who is here: the host name comes from the host's roster.
        let host_name = snap.host_name.clone();
        let mut people = card(p)
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(format!("In room ({count})")));
        for name in &snap.members {
            let is_you = name == &snap.cfg.name;
            let member_role = if is_you && snap.host {
                "You · Host".to_string()
            } else if name == &host_name {
                "Host".to_string()
            } else if is_you {
                "You".to_string()
            } else {
                "Guest".to_string()
            };
            people = people.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(crate::ui::avatar(None, px(24.), name, cx))
                    .child(div().flex_1().min_w_0().text_sm().text_color(p.text).truncate().child(if is_you { "You".to_string() } else { name.clone() }))
                    .child(div().text_xs().text_color(p.text2).child(member_role)),
            );
        }

        // Reactions and preset GIF bursts.
        let mut reactions = div().flex().items_center().gap_2().flex_wrap().child(div().text_xs().text_color(p.text2).child("React"));
        for (i, emoji) in ["❤️", "😂", "🔥", "🎉", "👍", "👎"].iter().enumerate() {
            let action = emoji.to_string();
            reactions = reactions.child(
                div()
                    .id(("room-react", i))
                    .flex_none()
                    .h_8()
                    .w_9()
                    .rounded_lg()
                    .border_1()
                    .border_color(p.border)
                    .bg(p.chip)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_base()
                    .child(*emoji)
                    .on_click(cx.listener(move |_, _, _, cx| room::react(&action, cx))),
            );
        }
        reactions = reactions.child(div().text_xs().text_color(p.text2).child("GIFs"));
        for (i, (label, token)) in [("Party", "gif:party"), ("LOL", "gif:lol"), ("Wow", "gif:wow")].iter().enumerate() {
            let action = token.to_string();
            reactions = reactions.child(button(("room-gif", i), label, p, false).on_click(cx.listener(move |_, _, _, cx| room::react(&action, cx))));
        }

        let settings = self.settings_open.then(|| self.settings_card(&snap, p, cx));
        let left = div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_w_0()
            .child(header)
            .child(now_playing)
            .child(reactions)
            .child(add_row)
            .children(settings);

        let queue_panel = self.queue_open.then(|| self.queue_card(&pe, &queue, qpos, p, cx));
        let mut right = div()
            .flex()
            .flex_col()
            .gap_4()
            .w(if two_col { px(380.) } else { primary_w })
            .flex_none()
            .child(people);
        if let Some(q) = queue_panel {
            right = right.child(q);
        }

        let body = if theater {
            div()
                .flex()
                .flex_col()
                .child(div().w_full().bg(black()).flex().justify_center().child(player_box))
                .child(
                    div()
                        .flex()
                        .gap_6()
                        .px_6()
                        .pb_24()
                        .when(!two_col, |d| d.flex_col())
                        .child(div().w(primary_w).flex_none().child(left))
                        .child(div().pt_3().child(right)),
                )
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .p_6()
                .w_full()
                .child(player_box)
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .w(primary_w)
                        .when(!two_col, |d| d.flex_col())
                        .child(left)
                        .child(right),
                )
        };

        div().id("room-tab").size_full().overflow_y_scroll().child(body).into_any_element()
    }
}


/// Floating emoji/GIF reactions drawn over the player by the app shell, so the
/// video surface (painted above the page) cannot hide them.
pub fn reactions_overlay(cx: &mut App) -> AnyElement {
    let reactions = crate::room::snapshot(cx).reactions;
    if reactions.is_empty() {
        return div().into_any_element();
    }
    let mut layer = div().absolute().inset_0();
    for r in reactions.iter().rev().take(12) {
        let glyph = reaction_glyph(&r.action);
        let x = 80.0 + (r.seq % 5) as f32 * 90.0;
        let start = 260.0 + (r.seq % 4) as f32 * 55.0;
        let id: SharedString = format!("rx-{}", r.seq).into();
        layer = layer.child(
            div()
                .absolute()
                .left(px(x))
                .top(px(start))
                .text_size(px(40.))
                .with_animation(id, Animation::new(std::time::Duration::from_millis(5000)), move |d, t| {
                    d.opacity((1.0 - t).max(0.0)).top(px(start - 170. * t))
                })
                .child(glyph.to_string()),
        );
    }
    layer.into_any_element()
}

fn reaction_glyph(action: &str) -> &'static str {
    match action {
        "gif:party" => "🎉",
        "gif:lol" => "😂",
        "gif:wow" => "😮",
        "❤️" => "❤️",
        "😂" => "😂",
        "🔥" => "🔥",
        "🎉" => "🎉",
        "👍" => "👍",
        "👎" => "👎",
        _ => "✨",
    }
}
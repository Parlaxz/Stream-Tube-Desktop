//! A room tab: the shared player plus the room queue. Sync targets only this
//! tab's player, so videos played in other tabs are never shared.
use crate::api::innertube;
use crate::player::controller::player;
use crate::room;
use crate::ui::cards::compact_row_with;
use crate::ui::*;
use gpui_kit::component::input::{Input, InputState};

pub struct RoomPage {
    link: Entity<InputState>,
}

impl RoomPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let link = cx.new(|cx| InputState::new(window, cx).placeholder("Paste a YouTube video link"));
        Self { link }
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

impl Render for RoomPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = pal(cx);
        let snap = room::snapshot(cx);
        let pe = crate::ui::app::room_player(cx).unwrap_or_else(|| player(cx));
        let (queue, qpos) = {
            let r = pe.read(cx);
            (r.queue.clone(), r.queue_pos)
        };

        let header = if let Some(info) = &snap.room {
            let role = if snap.host { "You are hosting" } else { "You are a guest" };
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(format!("Room {}", info.id)))
                        .child(div().text_xs().text_color(p.text2).child(role)),
                )
                .child(button("room-leave", "Leave room", p, false).on_click(cx.listener(|_, _, _w, cx| {
                    room::leave_room(cx);
                    crate::ui::app::open_social(cx);
                })))
        } else {
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().text_color(p.text2).child("No room is open."))
                .child(button("room-goto-social", "Open Social", p, true).on_click(cx.listener(|_, _, _w, cx| {
                    crate::ui::app::open_social(cx);
                })))
        };

        let add_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
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

        let mut list = div().id("room-queue").flex().flex_col().gap_1();
        if queue.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(p.text2)
                    .child("The room queue is empty. Paste a link above, or use Add to room on any video."),
            );
        }
        for (i, v) in queue.iter().enumerate() {
            let plr = pe.clone();
            let extra = vec![crate::ui::app::MenuItem::new("list-x", "Remove from queue", move |_, cx| {
                plr.update(cx, |p, cx| {
                    if i < p.queue.len() {
                        p.queue.remove(i);
                        if let Some(cur) = p.queue_pos.filter(|c| *c > i) {
                            p.queue_pos = Some(cur - 1);
                        }
                        cx.notify();
                    }
                })
            })];
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
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

        div()
            .id("room-tab")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_6()
                    .max_w(px(860.))
                    .child(header)
                    .child(add_row)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .rounded_xl()
                            .bg(p.surface)
                            .p_4()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(format!("Room queue ({})", queue.len())))
                            .child(list),
                    ),
            )
    }
}

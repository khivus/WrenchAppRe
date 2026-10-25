use gpui_kit::base::{Scrollbar, ScrollbarMode};
use gpui_kit::*;
use std::time::Instant;

#[derive(Default)]
pub(crate) struct WheelMotion {
    target: Option<f32>,
    expected: f32,
}

impl WheelMotion {
    pub(crate) fn cancel(&mut self) {
        self.target = None;
    }

    pub(crate) fn push(&mut self, current: f32, delta: f32, limit: f32) {
        let base = self
            .target
            .filter(|target| {
                (self.expected - current).abs() < 1. && (*target - current) * delta >= 0.
            })
            .unwrap_or(current);
        self.target = Some((base + delta).clamp(-limit.max(0.), 0.));
        self.expected = current;
    }

    pub(crate) fn step(&mut self, current: f32, limit: f32, elapsed: f32) -> Option<f32> {
        let target = self.target?;
        // A scrollbar drag, tab change, or native trackpad scroll takes precedence.
        if (current - self.expected).abs() > 1. {
            self.cancel();
            return None;
        }
        let target = target.clamp(-limit.max(0.), 0.);
        let next = current + (target - current) * (1. - (-elapsed.clamp(0.001, 0.1) / 0.045).exp());
        let next = if (target - next).abs() < 0.5 {
            self.cancel();
            target
        } else {
            self.target = Some(target);
            next
        };
        self.expected = next;
        Some(next)
    }
}

pub(crate) struct SmoothScroll {
    pub(crate) handle: ScrollHandle,
    pub(crate) horizontal: bool,
    pub(crate) motion: WheelMotion,
    pub(crate) frame_pending: bool,
    pub(crate) last_frame: Instant,
}

impl Default for SmoothScroll {
    fn default() -> Self {
        Self {
            handle: ScrollHandle::new(),
            horizontal: false,
            motion: WheelMotion::default(),
            frame_pending: false,
            last_frame: Instant::now(),
        }
    }
}

impl SmoothScroll {
    fn position(&self) -> f32 {
        let offset = self.handle.offset();
        if self.horizontal { offset.x } else { offset.y }.as_f32()
    }

    fn limit(&self) -> f32 {
        let offset = self.handle.max_offset();
        if self.horizontal { offset.x } else { offset.y }.as_f32()
    }

    pub(crate) fn reset(&mut self) {
        self.motion.cancel();
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    pub(crate) fn view<T: 'static, C: IntoElement, G: Fn(&mut T) -> &mut Self + Copy + 'static>(
        &self,
        id: &'static str,
        content: C,
        get: G,
        cx: &mut Context<T>,
    ) -> impl IntoElement + Styled + use<T, C, G> {
        div()
            .id(SharedString::from(format!("{id}-area")))
            .test_support()
            .flex()
            .flex_col()
            .relative()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(
                div()
                    .id(id)
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.handle)
                    .pr_4()
                    .child(content),
            )
            .child(self.wheel_layer(get, cx))
            .child(
                Scrollbar::vertical(&self.handle)
                    .id(SharedString::from(format!("{id}-scrollbar")))
                    .mode(ScrollbarMode::Always),
            )
    }

    pub(crate) fn wheel_layer<T: 'static, G: Fn(&mut T) -> &mut Self + Copy + 'static>(
        &self,
        get: G,
        cx: &mut Context<T>,
    ) -> impl IntoElement + use<T, G> {
        let scroll = self.handle.clone();
        let rendered_offset = scroll.offset();
        let wheel =
            cx.listener(
                move |this, event: &ScrollWheelEvent, window, cx| match event.delta {
                    ScrollDelta::Lines(delta) if delta.y != 0. || delta.x != 0. => {
                        let scroll = get(this);
                        let delta = if scroll.horizontal {
                            if delta.x != 0. { delta.x } else { delta.y }
                        } else {
                            delta.y
                        };
                        if delta == 0. {
                            return;
                        }
                        scroll
                            .motion
                            .push(scroll.position(), delta * 10., scroll.limit());
                        if !scroll.frame_pending {
                            scroll.last_frame = Instant::now();
                            scroll.frame_pending = true;
                            Self::schedule_frame(get, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    ScrollDelta::Pixels(_) => get(this).motion.cancel(),
                    _ => {}
                },
            );
        let cancel = cx.listener(move |this, _: &MouseDownEvent, _, _| get(this).motion.cancel());
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, cx| {
                // Layout can clamp the offset after a resize or data removal.
                // Refresh the visible rows and sticky header at that actual offset.
                if scroll.offset() != rendered_offset {
                    cx.notify(window.current_view());
                }
                let wheel_hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && wheel_hitbox.should_handle_scroll(window)
                    {
                        wheel(event, window, cx);
                    }
                });
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
                        cancel(event, window, cx);
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    fn schedule_frame<T: 'static>(
        get: impl Fn(&mut T) -> &mut Self + Copy + 'static,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        // This wakes the frame source directly and is safe outside painting,
        // including mouse input and the next-frame callback itself.
        cx.on_next_frame(window, move |this, window, cx| {
            let scroll = get(this);
            scroll.frame_pending = false;
            let elapsed = scroll.last_frame.elapsed().as_secs_f32();
            scroll.last_frame = Instant::now();
            if let Some(next) = scroll
                .motion
                .step(scroll.position(), scroll.limit(), elapsed)
            {
                let offset = scroll.handle.offset();
                scroll.handle.set_offset(if scroll.horizontal {
                    point(px(next), offset.y)
                } else {
                    point(offset.x, px(next))
                });
                cx.notify();
            }
            if scroll.motion.target.is_some() {
                scroll.frame_pending = true;
                Self::schedule_frame(get, window, cx);
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Instant, SmoothScroll};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::*;

    pub(crate) fn check_vertical<T: 'static>(
        view: &Entity<T>,
        get: fn(&mut T) -> &mut SmoothScroll,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scroll = view.update(cx, |view, _| get(view).handle.clone());
        let bounds = scroll.bounds();
        assert!(
            scroll.max_offset().y > px(30.),
            "scroll region must be constrained by its viewport"
        );
        let position = bounds.center();
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                ..Default::default()
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta: ScrollDelta::Lines(point(0., -3.)),
                ..Default::default()
            }),
            cx,
        );
        assert_eq!(scroll.offset().y, px(0.));
        for frame in 0..30 {
            view.update(cx, |view, _| {
                get(view).last_frame = Instant::now() - std::time::Duration::from_millis(16)
            });
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            if frame == 0 {
                assert!(scroll.offset().y < px(0.) && scroll.offset().y > px(-30.));
            }
        }
        assert_eq!(scroll.offset().y, px(-30.));
        view.update(cx, |view, _| get(view).reset());
        window.render_frame(cx);
        let from = point(bounds.right() - px(7.), bounds.top() + px(10.));
        window.drag(from, from + point(px(0.), px(80.)), cx);
        window.render_frame(cx);
        assert!(
            scroll.offset().y < px(-30.),
            "scrollbar must support dragging"
        );
        view.update(cx, |view, _| get(view).reset());
        window.render_frame(cx);
    }
}

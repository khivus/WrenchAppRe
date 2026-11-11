use super::{LayoutEditor, WindowRect, canvas::Mapping, model};
use eve_wrench_core::{SettingsKind, display_settings::scene};
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    *,
};
use rust_i18n::t;

impl LayoutEditor {
    pub(super) fn render_scene_overlays(
        &self,
        mapping: Mapping,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let screen = model::screen(&self.windows);
        let settings = &self.preview_settings;
        let mut elements = Vec::new();
        let rect = |x, y, width, height| WindowRect {
            x,
            y,
            width,
            height,
            screen_width: screen.width,
            screen_height: screen.height,
        };
        let color = cx.theme().primary;
        let muted = cx.theme().muted_foreground;
        if self.show_targets && !self.scene.docked() && settings.doc(SettingsKind::User).is_some() {
            for (i, r) in settings
                .targets()
                .rectangles(
                    screen.width,
                    screen.height,
                    settings.target_count,
                    &self.windows,
                )
                .into_iter()
                .enumerate()
            {
                let bounds = mapping.rect(&r);
                elements.push(
                    v_flex()
                        .id(SharedString::from(format!("preview-target-{i}")))
                        .test_support()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_settings(super::settings::Page::Targets, cx)
                        }))
                        .absolute()
                        .left(bounds.origin.x)
                        .top(bounds.origin.y)
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .border_1()
                        .border_color(color.opacity(0.7))
                        .bg(cx.theme().background.opacity(0.8))
                        .child(
                            div()
                                .text_size(px(14. * mapping.scale))
                                .text_color(color)
                                .child("◎"),
                        )
                        .child(
                            div()
                                .text_size(px(11. * mapping.scale))
                                .text_color(muted)
                                .child(format!("{} {}", t!("display.targetSample"), i + 1)),
                        )
                        .into_any_element(),
                );
            }
        }
        let count = if self.scene.docked() {
            0
        } else {
            self.fighter_count
        };
        if count > 0
            && let Some(doc) = settings.doc(SettingsKind::Char)
        {
            let (x, y) = scene::fighters_position(doc, screen.width, screen.height);
            let bounds = mapping.rect(&rect(x, y, 40 + 86 * count as i64, 266));
            elements.push(
                v_flex()
                    .id("preview-fighters")
                    .test_support()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_settings(super::settings::Page::Fighters, cx)
                    }))
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .overflow_hidden()
                    .border_1()
                    .border_color(color)
                    .bg(cx.theme().background.opacity(0.85))
                    .child(
                        div()
                            .text_size(px(11. * mapping.scale))
                            .child(format!("{} ({count})", t!("display.pages.fighters"))),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .items_stretch()
                            .gap(px(6. * mapping.scale))
                            .child(
                                div()
                                    .w(px(36. * mapping.scale))
                                    .text_size(px(11. * mapping.scale))
                                    .child("≡"),
                            )
                            .children((0..count).map(|i| {
                                v_flex()
                                    .id(SharedString::from(format!("preview-fighter-squadron-{i}")))
                                    .test_support()
                                    .w(px(80. * mapping.scale))
                                    .flex_none()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .border_color(color.opacity(0.5))
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(12. * mapping.scale))
                                            .child(format!("◇ {}", i + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11. * mapping.scale))
                                            .text_color(muted)
                                            .child("▣ ▣ ▣"),
                                    )
                            })),
                    )
                    .into_any_element(),
            );
        }
        if let Some(doc) = settings.doc(SettingsKind::Char) {
            let neocom = settings.neocom(self.neocom);
            let x = if neocom.side == eve_wrench_core::window_layout::NeocomSide::Left {
                0
            } else {
                screen.width - neocom.width
            };
            let icons = scene::neocom_buttons(doc);
            let buttons = std::iter::once("EVE".to_owned())
                .chain(std::iter::once("Pilot".to_owned()))
                .chain(icons)
                .collect::<Vec<_>>();
            for (i, id) in buttons.into_iter().enumerate() {
                let y = neocom.width * i as i64;
                if y + neocom.width > screen.height - 32 {
                    break;
                }
                let bounds = mapping.rect(&rect(x, y, neocom.width, neocom.width));
                let icon = match id.as_str() {
                    "Pilot" => IconName::User,
                    "inventory" | "assets" => IconName::Warehouse,
                    "wallet" => IconName::Wallet,
                    "market" => IconName::Store,
                    "fitting" => IconName::Wrench,
                    "map" | "map_beta" => IconName::Map,
                    "chat" | "mail" => IconName::MessageSquare,
                    "agency" => IconName::Search,
                    "job_board" => IconName::ListFilter,
                    _ => IconName::AppWindow,
                };
                elements.push(
                    div()
                        .id(SharedString::from(format!("preview-neocom-{i}")))
                        .test_support()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_settings(super::settings::Page::Neocom, cx)
                        }))
                        .absolute()
                        .left(bounds.origin.x)
                        .top(bounds.origin.y)
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_size(px(neocom.width as f32 * mapping.scale * 0.3))
                        .text_color(muted)
                        .child(Icon::new(icon).size(px(neocom.width as f32 * mapping.scale * 0.55)))
                        .into_any_element(),
                );
            }
        }
        elements
    }
}

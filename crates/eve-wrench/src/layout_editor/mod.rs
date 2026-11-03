mod canvas;
mod inspector;
mod model;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use eve_wrench_core::window_layout::{self, Neocom, WindowLayout, WindowRect, WindowState};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use self::canvas::{Drag, Pan, View};
use self::model::{History, Snapshot};
use crate::store::{Store, StoreEvent};
use crate::ui::{self, StyleExt as _};

actions!(layout_editor, [Undo, Redo, Save]);

const KEY_CONTEXT: &str = "LayoutEditor";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-z", Undo, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-shift-z", Redo, Some(KEY_CONTEXT)),
        KeyBinding::new("ctrl-y", Redo, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-s", Save, Some(KEY_CONTEXT)),
    ]);
}

pub fn embedded(path: String, window: &mut Window, cx: &mut App) -> Entity<LayoutEditor> {
    cx.new(|cx| LayoutEditor::new(PathBuf::from(path), window, cx))
}

pub struct LayoutSaved {
    pub path: String,
}
impl EventEmitter<LayoutSaved> for LayoutEditor {}

enum LoadState {
    Loading,
    Failed(SharedString),
    Ready,
}

// The client uses one layout for both, but shows some windows only docked
// and some only in space.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Location {
    InSpace,
    Docked,
}

// Rects where the client shows them, which is what the editor works on.
fn placed(mut layout: WindowLayout) -> WindowLayout {
    let neocom = layout.neocom;
    for window in &mut layout.windows {
        window.rect = window.rect.map(|r| neocom.place(r));
    }
    layout
}

// The four editable numbers of a rect, in inspector order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RectField {
    X,
    Y,
    Width,
    Height,
}

impl RectField {
    const ALL: [RectField; 4] = [
        RectField::X,
        RectField::Y,
        RectField::Width,
        RectField::Height,
    ];

    fn get(self, r: &WindowRect) -> i64 {
        match self {
            RectField::X => r.x,
            RectField::Y => r.y,
            RectField::Width => r.width,
            RectField::Height => r.height,
        }
    }

    fn set(self, r: &mut WindowRect, value: i64) {
        match self {
            RectField::X => r.x = value,
            RectField::Y => r.y = value,
            RectField::Width => r.width = value.max(model::MIN_WIDTH),
            RectField::Height => r.height = value.max(model::MIN_HEIGHT),
        }
    }

    fn label(self) -> &'static str {
        match self {
            RectField::X => "X",
            RectField::Y => "Y",
            RectField::Width => "W",
            RectField::Height => "H",
        }
    }
}

pub struct LayoutEditor {
    store: Entity<Store>,
    file_path: PathBuf,
    state: LoadState,
    // As shown in game, see `placed`
    windows: Vec<WindowState>,
    list_scroll: crate::scrolling::SmoothScroll,
    inspector_scroll: crate::scrolling::SmoothScroll,
    saved: Vec<WindowState>,
    neocom: Neocom,
    compact_default: bool,
    history: History,
    disk_changed: bool,
    saving: bool,
    focus_handle: FocusHandle,

    selected: Option<String>,
    hovered: Option<String>,
    drag: Option<Drag>,
    pan: Option<Pan>,
    view: View,
    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    snap: bool,
    show_closed: bool,
    location: Location,
    rect_inputs: [Entity<InputState>; 4],
    _subscriptions: Vec<Subscription>,
}

impl LayoutEditor {
    fn new(file_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::global(cx);
        let mut subscriptions =
            vec![
                cx.subscribe_in(&store, window, |this, _, event, window, cx| {
                    if let StoreEvent::DataChanged = event {
                        this.on_disk_changed(window, cx);
                    }
                }),
            ];

        let rect_inputs = RectField::ALL.map(|field| {
            let input = cx.new(|cx| InputState::new(window, cx));
            subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, _, cx| {
                    if let InputEvent::Change = event
                        && let Ok(value) = input.read(cx).value().trim().parse::<i64>()
                    {
                        this.typed_edit(format!("rect-{field:?}"), cx, |this| {
                            this.update_selected_rect(|r| field.set(r, value));
                        });
                    }
                },
            ));
            input
        });

        let mut editor = Self {
            store,
            file_path,
            state: LoadState::Loading,
            windows: Vec::new(),
            list_scroll: Default::default(),
            inspector_scroll: Default::default(),
            saved: Vec::new(),
            neocom: Neocom::default(),
            compact_default: false,
            history: History::default(),
            disk_changed: false,
            saving: false,
            focus_handle: cx.focus_handle(),
            selected: None,
            hovered: None,
            drag: None,
            pan: None,
            view: View::default(),
            canvas_bounds: Rc::new(Cell::new(Bounds::default())),
            snap: true,
            show_closed: false,
            location: Location::InSpace,
            rect_inputs,
            _subscriptions: subscriptions,
        };
        editor.load(window, cx);
        editor
    }

    pub fn is_dirty(&self) -> bool {
        self.windows != self.saved
    }

    pub fn has_pending_edits(&self) -> bool {
        self.is_dirty() || self.saving
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            windows: self.windows.clone(),
            selected: self.selected.clone(),
        }
    }

    fn selected_window(&self) -> Option<&WindowState> {
        let key = self.selected.as_ref()?;
        self.windows.iter().find(|w| &w.key == key)
    }

    fn update_selected_rect(&mut self, f: impl FnOnce(&mut WindowRect)) {
        let Some(window) = self.selected_window() else {
            return;
        };
        let Some(mut rect) = window.rect else {
            return;
        };
        let key = window.key.clone();
        f(&mut rect);
        model::set_rect(&mut self.windows, &key, rect);
    }

    fn update_selected(&mut self, f: impl FnOnce(&mut WindowState)) {
        let Some(key) = self.selected.clone() else {
            return;
        };
        if let Some(window) = self.windows.iter_mut().find(|w| w.key == key) {
            f(window);
        }
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state = LoadState::Loading;
        self.disk_changed = false;
        cx.notify();
        let path = self.file_path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { window_layout::read_window_layout(&path) })
                .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(layout) => {
                        let layout = placed(layout);
                        this.neocom = layout.neocom;
                        this.compact_default = layout.compact_default;
                        this.saved = layout.windows.clone();
                        this.windows = layout.windows;
                        this.history.clear();
                        this.state = LoadState::Ready;
                    }
                    Err(e) => this.state = LoadState::Failed(e.into()),
                }
                this.after_structural_change(window, cx);
            })
            .ok();
        })
        .detach();
    }

    // Another window changed settings: follow the file unless that would
    // discard unsaved edits, in which case offer a reload instead.
    fn on_disk_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.file_path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let disk = cx
                .background_spawn(async move { window_layout::read_window_layout(&path) })
                .await;
            this.update_in(cx, |this, window, cx| {
                if this.saving {
                    return;
                }
                let Ok(layout) = disk.map(placed) else {
                    return;
                };
                this.neocom = layout.neocom;
                this.compact_default = layout.compact_default;
                let disk = layout.windows;
                if disk == this.saved || !matches!(this.state, LoadState::Ready) {
                    return;
                }
                if this.is_dirty() {
                    this.disk_changed = true;
                } else {
                    this.saved = disk.clone();
                    this.windows = disk;
                    this.history.clear();
                    this.after_structural_change(window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // Keeps the selection valid and pushes the model into the text inputs
    // after any change that didn't come from typing.
    fn after_structural_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_window().is_none() {
            self.selected = None;
        }
        self.sync_inputs(window, cx);
        cx.notify();
    }

    fn sync_inputs(&self, window: &mut Window, cx: &mut Context<Self>) {
        let rect = self.selected_window().and_then(|w| w.rect);
        for (field, input) in RectField::ALL.iter().zip(&self.rect_inputs) {
            let text = rect.map(|r| field.get(&r).to_string()).unwrap_or_default();
            input.update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }

    // An undoable change made through buttons, toggles or the canvas.
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Self)) {
        self.history.record(self.snapshot(), None);
        f(self);
        self.after_structural_change(window, cx);
    }

    // An undoable change typed into a field; one field's keystrokes collapse
    // into a single step and the field itself is left alone.
    fn typed_edit(&mut self, key: String, cx: &mut Context<Self>, f: impl FnOnce(&mut Self)) {
        let before = self.snapshot();
        f(self);
        if self.windows != before.windows {
            self.history.record(before, Some(key));
        }
        cx.notify();
    }

    fn select(&mut self, key: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if key != self.selected {
            self.selected = key;
            self.after_structural_change(window, cx);
        }
    }

    fn restore(&mut self, snapshot: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.windows = snapshot.windows;
        self.selected = snapshot.selected;
        self.after_structural_change(window, cx);
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.history.undo(self.snapshot()) {
            self.restore(previous, window, cx);
        }
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.history.redo(self.snapshot()) {
            self.restore(next, window, cx);
        }
    }

    fn save_action(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_dirty() && !self.saving {
            self.save(window, cx);
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(window, cx, |this| this.windows = this.saved.clone());
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.windows.clone();
        let neocom = self.neocom;
        let payload: Vec<WindowState> = shown
            .iter()
            .cloned()
            .map(|mut w| {
                w.rect = w.rect.map(|r| neocom.store(r));
                w
            })
            .collect();
        let backup = self.store.read(cx).config().auto_backup;
        let path = self.file_path.clone();
        self.saving = true;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    window_layout::write_window_layout(&path, &payload, backup)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.saved = shown;
                        this.disk_changed = false;
                        ui::notify_success(
                            t!("layoutEditor.saved"),
                            t!("layoutEditor.savedDesc"),
                            window,
                            cx,
                        );
                        this.store.update(cx, |store, cx| store.reload(cx));
                        cx.emit(LayoutSaved {
                            path: this.file_path.to_string_lossy().into_owned(),
                        });
                    }
                    Err(e) => ui::notify_error(t!("layoutEditor.saveFailed"), e, window, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl LayoutEditor {
    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dirty = self.is_dirty();
        h_flex()
            .flex_none()
            .h(px(52.))
            .gap_2()
            .px_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("layout-undo")
                    .ghost()
                    .small()
                    .icon(IconName::Undo)
                    .tooltip(t!("formationEditor.undo").to_string())
                    .disabled(!self.history.can_undo())
                    .on_click(cx.listener(|this, _, window, cx| this.undo(&Undo, window, cx))),
            )
            .child(
                Button::new("layout-redo")
                    .ghost()
                    .small()
                    .icon(IconName::Redo)
                    .tooltip(t!("formationEditor.redo").to_string())
                    .disabled(!self.history.can_redo())
                    .on_click(cx.listener(|this, _, window, cx| this.redo(&Redo, window, cx))),
            )
            .when(dirty, |this| {
                this.child(
                    h_flex()
                        .ml_2()
                        .gap_1p5()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(div().size_1p5().rounded_full().bg(ui::amber(cx)))
                        .child(t!("formationEditor.unsaved").to_string()),
                )
            })
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("layoutEditor.closeClient").to_string()),
            )
            .child(
                Button::new("layout-reset")
                    .outline()
                    .small()
                    .label(t!("formationEditor.reset").to_string())
                    .disabled(self.saving || !dirty)
                    .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
            )
            .child(
                Button::new("layout-save")
                    .primary()
                    .small()
                    .min_w(px(120.))
                    .label(t!("layoutEditor.save").to_string())
                    .loading(self.saving)
                    .disabled(self.saving || !dirty)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            )
    }
}

impl Render for LayoutEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.state {
            LoadState::Loading => div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(t!("formationEditor.loading").to_string())
                .into_any_element(),
            LoadState::Failed(error) => v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .p_8()
                .text_center()
                .child(
                    div()
                        .font_medium()
                        .child(t!("layoutEditor.loadFailed").to_string()),
                )
                .child(
                    div()
                        .max_w(px(512.))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(error.clone()),
                )
                .child(
                    Button::new("retry")
                        .outline()
                        .small()
                        .label(t!("common.refresh").to_string())
                        .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                )
                .into_any_element(),
            LoadState::Ready => v_flex()
                .flex_1()
                .min_h_0()
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .items_stretch()
                        .child(self.render_list(cx))
                        .child(self.render_canvas(cx))
                        .child(self.render_inspector(cx)),
                )
                .child(self.render_footer(cx))
                .into_any_element(),
        };

        v_flex()
            .id("layout-editor")
            .test_support()
            .size_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save_action))
            // Drags and pans keep going when the pointer leaves the canvas
            .on_mouse_move(cx.listener(Self::on_drag_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_drag_end))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_drag_end))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .when(self.disk_changed, |this| {
                let amber = ui::amber(cx);
                this.child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .px_4()
                        .py_1p5()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .bg(amber.opacity(0.1))
                        .child(
                            div()
                                .text_xs()
                                .text_color(amber)
                                .child(t!("formationEditor.fileChanged").to_string()),
                        )
                        .child(
                            Button::new("layout-reload")
                                .outline()
                                .xsmall()
                                .label(t!("formationEditor.reload").to_string())
                                .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                        ),
                )
            })
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutEditor, WindowRect, WindowState, window_layout};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size,
    };
    use std::path::PathBuf;

    #[gpui_kit::test]
    fn layout_list_and_inspector_scroll_smoothly_and_independently(cx: &mut TestAppContext) {
        let (_, _, files) = crate::main_window::editors_tests::setup(cx);
        let rect = WindowRect {
            x: 100,
            y: 100,
            width: 300,
            height: 400,
            screen_width: 1920,
            screen_height: 1080,
        };
        let mut windows: Vec<_> = (0..60)
            .map(|i| WindowState {
                key: format!("bytes:overview_{i}"),
                rect: Some(rect),
                open: true,
                locked: false,
                minimized: false,
                compact: None,
                light_background: None,
                stack: None,
            })
            .collect();
        windows.push(WindowState {
            key: "bytes:100".into(),
            rect: Some(rect),
            open: true,
            locked: false,
            minimized: false,
            compact: None,
            light_background: None,
            stack: None,
        });
        windows.extend((0..60).map(|i| WindowState {
            key: format!("utf8:chatchannel_{i}"),
            rect: Some(rect),
            open: true,
            locked: false,
            minimized: false,
            compact: None,
            light_background: None,
            stack: Some("bytes:100".into()),
        }));
        let path = files.character.clone();
        // Stacks are read from the file; build their stored map alongside the layout fixture.
        let bytes = std::fs::read(&path).unwrap();
        let decoded = blue_marshal::decode(&bytes).unwrap();
        let mut root = blue_marshal::to_json(&decoded.value);
        let stacks: serde_json::Map<_, _> = windows
            .iter()
            .filter_map(|w| {
                w.stack
                    .as_ref()
                    .map(|stack| (w.key.clone(), serde_json::json!(stack)))
            })
            .collect();
        root["bytes:windows"] =
            serde_json::json!({ "bytes:stacksWindows": { "tuple": ["long:1", stacks] } });
        std::fs::write(
            &path,
            blue_marshal::encode(
                &blue_marshal::from_json(&root).unwrap(),
                &blue_marshal::EncodeOptions::default(),
            )
            .unwrap(),
        )
        .unwrap();
        window_layout::write_window_layout(std::path::Path::new(&path), &windows, false).unwrap();
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1440.), px(650.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| LayoutEditor::new(PathBuf::from(path), window, cx)),
            )
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |editor, cx| {
                editor.select(Some("bytes:100".into()), window, cx)
            });
            window.render_frame(cx);

            crate::scrolling::tests::check_vertical(&editor, |e| &mut e.list_scroll, window, cx);
            assert_eq!(editor.read(cx).inspector_scroll.handle.offset().y, px(0.));

            crate::scrolling::tests::check_vertical(
                &editor,
                |e| &mut e.inspector_scroll,
                window,
                cx,
            );
            assert_eq!(editor.read(cx).list_scroll.handle.offset().y, px(0.));
        })
        .unwrap();
    }
}

use super::*;

// A settings-panel scroll invalidates its ancestors. Cache the canvas itself,
// so those frames don't rebuild every sample row and window on the desktop.
pub(super) struct CanvasPreview {
    parent: WeakEntity<LayoutEditor>,
    #[cfg(test)]
    pub(super) builds: usize,
}
impl Render for CanvasPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        {
            self.builds += 1;
        }
        self.parent
            .update(cx, |parent, cx| parent.render_canvas(cx).into_any_element())
            .unwrap_or_else(|_| div().into_any_element())
    }
}

#[derive(PartialEq)]
pub(super) struct CanvasStamp {
    windows: Vec<WindowState>,
    activity: WindowActivity,
    selected: Option<String>,
    hovered: Option<String>,
    hidden: HashSet<String>,
    overview_tabs: std::collections::HashMap<String, i64>,
    stack_tabs: std::collections::HashMap<String, String>,
    view: View,
    scene: Scene,
    bounds: Bounds<Pixels>,
    neocom: Neocom,
    compact: bool,
    snap: bool,
    show_all: bool,
    targets: bool,
    settings_open: bool,
    settings_revision: u64,
}
impl LayoutEditor {
    pub(super) fn cached_canvas(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let stamp = CanvasStamp {
            windows: self.windows.clone(),
            activity: self.activity.clone(),
            selected: self.selected.clone(),
            hovered: self.hovered.clone(),
            hidden: self.preview_hidden.clone(),
            overview_tabs: self.preview_overview_tabs.clone(),
            stack_tabs: self.preview_stack_tabs.clone(),
            view: self.view,
            scene: self.scene,
            bounds: self.canvas_bounds.get(),
            neocom: self.neocom,
            compact: self.compact_default,
            snap: self.snap,
            show_all: self.show_all_windows,
            targets: self.show_targets,
            settings_open: self.settings_open,
            settings_revision: self.settings_revision,
        };
        if self.canvas_preview.is_none() {
            let parent = cx.entity().downgrade();
            self.canvas_preview = Some(cx.new(|_| CanvasPreview {
                parent,
                #[cfg(test)]
                builds: 0,
            }));
        }
        let view = self.canvas_preview.as_ref().unwrap();
        if self.canvas_stamp.as_ref() != Some(&stamp) {
            self.canvas_stamp = Some(stamp);
            view.update(cx, |_, cx| cx.notify());
        }
        view.clone()
            .cached(div().flex_1().h_full().min_w_0().style().clone())
            .into_any_element()
    }
}

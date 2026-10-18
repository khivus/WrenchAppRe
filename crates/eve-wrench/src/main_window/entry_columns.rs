use eve_wrench_core::{SettingsEntry, SettingsKind, config::CharacterColumns};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::*;
use rust_i18n::t;

use super::title_bar::view_action;
use super::{MainView, Sort, SortColumn};
use crate::table::Column;
use crate::ui;

const OPTIONAL_COLUMNS: [SortColumn; 3] = [
    SortColumn::Corporation,
    SortColumn::Alliance,
    SortColumn::CreationDate,
];

pub(super) struct EntryTableLayout {
    pub fields: Vec<SortColumn>,
    pub columns: Vec<Column>,
    pub min_width: Pixels,
}

impl EntryTableLayout {
    pub fn new(kind: SettingsKind, visible: CharacterColumns) -> Self {
        let mut fields = match kind {
            SettingsKind::Char => vec![SortColumn::Name, SortColumn::Id],
            SettingsKind::User => vec![SortColumn::Id, SortColumn::Name],
        };
        if kind == SettingsKind::Char {
            fields.extend(
                OPTIONAL_COLUMNS
                    .into_iter()
                    .filter(|column| is_visible(visible, *column)),
            );
        }
        fields.push(SortColumn::Time);
        let mut min_width = px(440.);
        let mut columns = vec![Column::fixed(px(28.))];
        for field in &fields {
            columns.push(match field {
                SortColumn::Name => Column::fill(),
                SortColumn::Id => Column::fixed(px(100.)),
                SortColumn::Time => Column::fixed(px(88.)),
                SortColumn::Corporation | SortColumn::Alliance => {
                    min_width += px(160.);
                    Column::fixed(px(160.))
                }
                SortColumn::CreationDate => {
                    min_width += px(112.);
                    Column::fixed(px(112.))
                }
            });
        }
        columns.push(Column::fixed(px(80.)).end());
        Self {
            fields,
            columns,
            min_width,
        }
    }
}

pub(super) fn column_id(column: SortColumn) -> &'static str {
    match column {
        SortColumn::Name => "name",
        SortColumn::Id => "id",
        SortColumn::Time => "time",
        SortColumn::Corporation => "corporation",
        SortColumn::Alliance => "alliance",
        SortColumn::CreationDate => "creation-date",
    }
}

pub(super) fn column_label(column: SortColumn, kind: SettingsKind) -> String {
    match column {
        SortColumn::Name if kind == SettingsKind::User => t!("table.customName"),
        SortColumn::Name => t!("table.name"),
        SortColumn::Id if kind == SettingsKind::User => t!("table.accountId"),
        SortColumn::Id => t!("table.characterId"),
        SortColumn::Time => t!("table.modified"),
        SortColumn::Corporation => t!("table.corporation"),
        SortColumn::Alliance => t!("table.alliance"),
        SortColumn::CreationDate => t!("table.creationDate"),
    }
    .to_string()
}

fn is_visible(visible: CharacterColumns, column: SortColumn) -> bool {
    match column {
        SortColumn::Corporation => visible.corporation,
        SortColumn::Alliance => visible.alliance,
        SortColumn::CreationDate => visible.creation_date,
        _ => true,
    }
}

pub(super) fn entry_column_text(entry: &SettingsEntry, column: SortColumn) -> String {
    let character = entry.character.as_ref();
    let text = match column {
        SortColumn::Name if entry.kind == SettingsKind::User => entry.alias.as_deref(),
        SortColumn::Name => Some(entry.display_name.as_str()),
        SortColumn::Id => Some(entry.id.as_str()),
        SortColumn::Time => return ui::relative_time(entry.modified_time).to_string(),
        SortColumn::Corporation => character.and_then(|info| info.corporation.as_deref()),
        SortColumn::Alliance => character.and_then(|info| info.alliance.as_deref()),
        SortColumn::CreationDate => character.and_then(|info| info.creation_date()),
    };
    text.filter(|text| !text.is_empty())
        .unwrap_or("—")
        .to_string()
}

impl MainView {
    fn toggle_character_column(
        &mut self,
        column: SortColumn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !OPTIONAL_COLUMNS.contains(&column) {
            return;
        }
        let result = self.store.update(cx, |store, cx| {
            store.update_config(cx, |config| {
                let visible = &mut config.character_columns;
                let value = match column {
                    SortColumn::Corporation => &mut visible.corporation,
                    SortColumn::Alliance => &mut visible.alliance,
                    SortColumn::CreationDate => &mut visible.creation_date,
                    _ => unreachable!(),
                };
                *value = !*value;
            })
        });
        if let Err(error) = result {
            ui::notify_error(t!("table.columnsSaveFailed"), error, window, cx);
            return;
        }
        let visible = self.store.read(cx).config().character_columns;
        if !is_visible(visible, self.character_sort.column) {
            self.character_sort = Sort {
                column: SortColumn::Id,
                ascending: true,
            };
        }
        let handle = &self.character_scroll.handle;
        handle.set_offset(point(px(0.), handle.offset().y));
        cx.notify();
    }

    pub(super) fn character_columns_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let visible = self.store.read(cx).config().character_columns;
        let view = cx.entity().downgrade();
        div().id("character-columns-button").test_support().child(
            Button::new("character-columns")
                .ghost()
                .xsmall()
                .icon(IconName::Settings)
                .label(t!("table.columns").to_string())
                .dropdown_menu(move |menu, _, _| {
                    OPTIONAL_COLUMNS.into_iter().fold(menu, |menu, column| {
                        menu.item(
                            PopupMenuItem::element(move |_, _| {
                                div()
                                    .id(SharedString::from(format!(
                                        "column-option-{}",
                                        column_id(column)
                                    )))
                                    .test_support()
                                    .child(column_label(column, SettingsKind::Char))
                            })
                            .checked(is_visible(visible, column))
                            .on_click(view_action(
                                &view,
                                move |this, window, cx| {
                                    this.toggle_character_column(column, window, cx)
                                },
                            )),
                        )
                    })
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MainView, SortColumn, profiles::tests::sample_data};
    use super::{EntryTableLayout, entry_column_text};
    use crate::store::Store;
    use eve_wrench_core::{
        CharacterDetails, SettingsKind,
        config::{CharacterColumns, Config},
    };
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, SharedString, TestAppContext, WindowBounds, WindowOptions, point, px,
        size,
    };

    #[test]
    fn defaults_keep_ids_and_names_separate_and_extra_columns_are_optional() {
        let hidden = CharacterColumns::default();
        assert!(
            EntryTableLayout::new(SettingsKind::Char, hidden).fields
                == [SortColumn::Name, SortColumn::Id, SortColumn::Time]
        );
        assert!(
            EntryTableLayout::new(SettingsKind::User, hidden).fields
                == [SortColumn::Id, SortColumn::Name, SortColumn::Time]
        );
        let all = EntryTableLayout::new(
            SettingsKind::Char,
            CharacterColumns {
                corporation: true,
                alliance: true,
                creation_date: true,
            },
        );
        assert_eq!(all.fields.len(), 6);
        assert_eq!(all.columns.len(), 8);
        assert_eq!(all.min_width, px(872.));
        let data = sample_data();
        let account = &data.servers[0].profiles[0].accounts[0];
        assert_eq!(entry_column_text(account, SortColumn::Name), "—");
        assert_eq!(entry_column_text(account, SortColumn::Id), "2");
    }

    #[gpui_kit::test]
    fn titles_do_not_sort_and_column_menu_saves_and_scrolls_with_headers(cx: &mut TestAppContext) {
        let mut data = sample_data();
        let profile = &mut data.servers[0].profiles[1];
        profile.accounts[0].alias = Some("Zulu".into());
        profile.accounts[0].display_name = "Zulu".into();
        profile.accounts[1].alias = Some("Alpha".into());
        profile.accounts[1].display_name = "Alpha".into();
        for entry in &mut profile.characters {
            entry.character = Some(CharacterDetails {
                name: format!("Pilot {}", entry.id),
                corporation: Some("Test Corporation".into()),
                alliance: Some("Test Alliance".into()),
                created_at: Some("2008-05-06T07:08:09Z".into()),
                ..Default::default()
            });
            entry.display_name = format!("Pilot {}", entry.id);
        }
        assert_eq!(
            entry_column_text(&profile.characters[0], SortColumn::CreationDate),
            "2008-05-06"
        );
        let profile_path = profile.path.clone();
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1200.), px(700.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| MainView::new(window, cx)),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("table-title-characters", cx);
            assert!(view.read(cx).character_sort.column == SortColumn::Id);
            assert!(view.read(cx).character_sort.ascending);
            window.click(
                SharedString::from(format!("sort-name-{profile_path}-User")),
                cx,
            );
            assert!(view.read(cx).account_sort.column == SortColumn::Name);
            window.click("table-title-accounts", cx);
            assert!(view.read(cx).account_sort.column == SortColumn::Name);
            window.click(
                SharedString::from(format!("sort-id-{profile_path}-User")),
                cx,
            );
            assert!(view.read(cx).account_sort.column == SortColumn::Id);
            assert!(view.read(cx).account_sort.ascending);
        })
        .unwrap();
        // End each app update so menu-dismiss events run before opening it again.
        for column in ["corporation", "alliance", "creation-date"] {
            cx.update_window(handle, |_, window, cx| {
                window.click("character-columns-button", cx)
            })
            .unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.click(SharedString::from(format!("column-option-{column}")), cx);
            })
            .unwrap();
        }
        let config_file = cx
            .update_window(handle, |_, window, cx| {
                let store = Store::global(cx);
                let config_file = store.read(cx).locations().config_file();
                let columns = Config::load(&config_file).character_columns;
                assert!(columns.corporation && columns.alliance && columns.creation_date);
                let bounds = view.read(cx).character_scroll.handle.bounds();
                assert!(view.read(cx).character_scroll.handle.max_offset().x > px(0.));
                let from = point(bounds.left() + px(10.), bounds.bottom() - px(7.));
                window.drag(from, point(bounds.right() - px(20.), from.y), cx);
                window.render_frame(cx);
                assert!(view.read(cx).character_scroll.handle.offset().x < px(0.));
                assert_eq!(view.read(cx).account_scroll.handle.offset().x, px(0.));
                let created_header =
                    SharedString::from(format!("sort-creation-date-{profile_path}-Char"));
                assert!(window.find(created_header.clone()).visible());
                window.click(created_header, cx);
                assert!(view.read(cx).character_sort.column == SortColumn::CreationDate);
                config_file
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.click("character-columns-button", cx)
        })
        .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.click("column-option-creation-date", cx)
        })
        .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(!Config::load(&config_file).character_columns.creation_date);
            assert!(view.read(cx).character_sort.column == SortColumn::Id);
            std::fs::remove_file(&config_file).unwrap();
            std::fs::remove_dir(config_file.parent().unwrap()).unwrap();
        })
        .unwrap();
    }
}

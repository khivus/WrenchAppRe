use super::*;

impl DisplayEditor {
    pub(super) fn render_chats(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.doc(SettingsKind::User).is_none() {
            return hint("display.chooseAccount", cx);
        }
        let Some(id) = self
            .chats
            .iter()
            .find(|id| self.selected_chat.as_ref() == Some(id))
            .or_else(|| self.chats.first())
            .cloned()
        else {
            return hint("display.noChats", cx);
        };
        let name = chat::channel(&id).unwrap().to_owned();
        let options = self
            .chats
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    format!(
                        "{} · {}",
                        super::super::model::window_name(id),
                        chat::channel(id).unwrap()
                    ),
                )
            })
            .collect();
        let layout = chat::ChatLayout::read(self.doc(SettingsKind::User), &id);
        let toggle_name = name.clone();
        v_flex()
            .gap_3()
            .child(hint("display.chatHint", cx))
            .child(self.picker(
                "chat-window",
                super::super::model::window_name(&id),
                options,
                id,
                |this, id, cx| {
                    this.selected_chat = Some(id);
                    cx.notify();
                },
                cx,
            ))
            .child(self.input(
                Field::ChatWidth(name.clone()),
                t!("display.chatMemberWidth").to_string(),
                window,
                cx,
            ))
            .child(self.input(
                Field::ChatInput(name),
                t!("display.chatInputHeight").to_string(),
                window,
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .child(t!("display.chatMembers").to_string()),
                    )
                    .child(
                        Switch::new("chat-show-members")
                            .small()
                            .checked(layout.show_members)
                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                let result = this.doc_mut(SettingsKind::User).and_then(|doc| {
                                    chat::ChatLayout::set(
                                        doc,
                                        &toggle_name,
                                        "visible",
                                        i64::from(*checked),
                                    )
                                });
                                this.error = result.err();
                                cx.emit(DisplayChanged);
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}

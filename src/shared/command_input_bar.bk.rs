use makepad_widgets::*;
use unicode_segmentation::UnicodeSegmentation;

live_design! {
    link widgets;
    use link::widgets::*;
    use link::theme::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;

    // // Template defining how each user list item should look
    // pub UserListItem = <View> {
    //     width: Fill,
    //     height: Fit,
    //     padding: {left: 8., right: 8., top: 4., bottom: 4.}
    //     show_bg: true
    //     draw_bg: {color: #fff}
    //     flow: Right
    //     spacing: 8.0
    //     align: {y: 0.5}

    //     avatar = <Avatar> {
    //         width: 24,
    //         height: 24,
    //         text_view = { text = { draw_text: {
    //             text_style: { font_size: 12.0 }
    //         }}}
    //     }

    //     label = <Label> {
    //         height: Fit,
    //         draw_text: {
    //             color: #000,
    //             text_style: {font_size: 14.0}
    //         }
    //     }

    //     matrix_url = <Label> {
    //         height: Fit,
    //         draw_text: {
    //             color: #666,
    //             text_style: {font_size: 12.0}
    //         }
    //     }
    // }

    // Main component design
    pub CommandInputBar = {{CommandInputBar}} {
        flow: Down,
        height: Fit,

        keyboard_focus_color: (THEME_COLOR_CTRL_HOVER),
        pointer_hover_color: (THEME_COLOR_CTRL_HOVER * 0.85),

        // Popup menu that appears when trigger character is typed
        popup = <RoundedView> {
            flow: Down,
            width: Fill
            height: Fit
            visible: false

            // Scrollable list of users
            list = <PortalList> {
                width: Fill
                height: Fit
                flow: Down
                UserListItem = <View> {
                    width: Fill,
                    height: Fit,
                    padding: {left: 8., right: 8., top: 4., bottom: 4.}
                    show_bg: true
                    draw_bg: {color: #fff}
                    flow: Right
                    spacing: 8.0
                    align: {y: 0.5}

                    avatar = <Avatar> {
                        width: 24,
                        height: 24,
                        text_view = { text = { draw_text: {
                            text_style: { font_size: 12.0 }
                        }}}
                    }

                    label = <Label> {
                        height: Fit,
                        draw_text: {
                            color: #000,
                            text_style: {font_size: 14.0}
                        }
                    }

                    matrix_url = <Label> {
                        height: Fit,
                        draw_text: {
                            color: #666,
                            text_style: {font_size: 12.0}
                        }
                    }
                }
            }
        }

        // Main text input area
        persistent = <RoundedView> {
            flow: Down,
            height: Fit,
            top = <View> { height: Fit }
            center = <RoundedView> {
                height: Fit,
                left = <View> { width: Fit, height: Fit }
                text_input = <TextInput> {
                    width: Fill,
                    draw_bg: {
                        color: (COLOR_PRIMARY)
                        instance radius: 2.0
                        instance border_width: 0.8
                        instance border_color: #D0D5DD
                    }
                    draw_text: {
                        color: (#000),
                        text_style: <MESSAGE_TEXT_STYLE>{}
                    }
                }
                right = <View> { width: Fit, height: Fit }
            }
            bottom = <View> { height: Fit }
        }
    }
}

// Internal actions used for component state management
#[derive(Debug, Copy, Clone, DefaultNone)]
enum InternalAction {
    ShouldBuildItems,  // Triggered when items need to be rebuilt (e.g. after filtering)
    ItemSelected,      // Triggered when user selects an item
    None,
}

// Main component implementation
#[derive(Live, Widget)]
pub struct CommandInputBar {
    #[deref]
    deref: View,

    #[live]
    pub trigger: Option<String>,  // The character that triggers the mention popup (@)

    #[live]
    pub keyboard_focus_color: Vec4,
    #[live]
    pub pointer_hover_color: Vec4,

    // Component state tracking
    #[rust]
    is_text_input_focus_pending: bool,
    #[rust]
    keyboard_focus_index: Option<usize>,
    #[rust]
    pointer_hover_index: Option<usize>,
    #[rust]
    selectable_widgets: Vec<WidgetRef>,
    #[rust]
    last_selected_widget: WidgetRef,
}

// Implement required Widget trait
impl Widget for CommandInputBar {
    fn text(&self) -> String {
        self.text_input_ref().text()
    }

    fn set_text(&mut self, v: &str) {
        self.text_input_ref().set_text(v);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.update_highlights(cx);

        while !self.deref.draw_walk(cx, scope, walk).is_done() {}

        if self.is_text_input_focus_pending {
            self.is_text_input_focus_pending = false;
            self.text_input_ref().set_key_focus(cx);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        self.deref.handle_event(cx, event, scope);

        match event {
            Event::TextInput(input_event) => {
                // if cx.has_key_focus(self.text_input_ref().area()) {
                    self.on_text_inserted(cx, scope, &input_event.input);
                // }
            }

            Event::KeyDown(key_event) => {
                if self.view(id!(popup)).visible()
                {
                    match key_event.key_code {
                        KeyCode::ArrowDown => self.move_list_selection(cx, 1),
                        KeyCode::ArrowUp => self.move_list_selection(cx, -1),
                        KeyCode::ReturnKey => self.select_focused_item(cx, scope),
                        KeyCode::Escape => {
                            self.hide_popup(cx);
                            self.redraw(cx);
                        }
                        _ => {}
                    }
                }
            }

            Event::Actions(actions) => {
                self.handle_list_item_actions(cx, scope, actions);
            }

            _ => {}
        }
    }
}

// Implement LiveHook for proper widget initialization
impl LiveHook for CommandInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        self.text_input_ref().set_key_focus(cx);
    }
}

// Main component functionality
impl CommandInputBar {
    // Called when text is inserted into the input field
    fn on_text_inserted(&mut self, cx: &mut Cx, scope: &mut Scope, inserted: &str) {
        log!("Text inserted: {:?}", inserted);
        if inserted.graphemes(true).last() == self.trigger_grapheme() {
            log!("Trigger character detected");
            self.show_popup(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                InternalAction::ShouldBuildItems,
            );
        }
    }

    // Handles mouse interaction with list items
    fn handle_list_item_actions(&mut self, cx: &mut Cx, scope: &mut Scope, actions: &Actions) {
        let mut selected_by_click = None;
        let mut should_redraw = false;

        for (idx, item) in self.selectable_widgets.iter().enumerate() {
            let item = item.as_view();

            if item.finger_down(actions).map(|fe| fe.tap_count == 1).unwrap_or(false) {
                selected_by_click = Some((&*item).clone());
            }

            if item.finger_hover_out(actions).is_some() && Some(idx) == self.pointer_hover_index {
                self.pointer_hover_index = None;
                should_redraw = true;
            }

            if item.finger_hover_in(actions).is_some() {
                self.pointer_hover_index = Some(idx);
                should_redraw = true;
            }
        }

        if should_redraw {
            self.redraw(cx);
        }

        if let Some(selected) = selected_by_click {
            self.select_item(cx, scope, selected);
        }
    }

    // Handles keyboard navigation of the list
    fn move_list_selection(&mut self, cx: &mut Cx, delta: i32) {
        let current_index = self.keyboard_focus_index.unwrap_or(0);
        let new_index = (current_index as i32 + delta)
            .clamp(0, (self.selectable_widgets.len() as i32) - 1) as usize;

        self.keyboard_focus_index = Some(new_index);

        // Use smooth scrolling to ensure selected item is visible
        self.portal_list(id!(list))
            .smooth_scroll_to(cx, new_index, 50.0, Some(5));

        self.redraw(cx);
    }

    // Gets text after the trigger character for filtering
    fn get_trigger_text(&self) -> String {
        let text = self.text();
        let cursor_pos = self.text_input_ref()
            .borrow()
            .map_or(0, |p| p.get_cursor().head.index);

        let trigger_pos = text[..cursor_pos].rfind('@')
            .filter(|&pos| !text[pos..cursor_pos].contains(char::is_whitespace));

        trigger_pos.map_or(String::new(), |pos| text[pos + 1..cursor_pos].to_string())
    }

    // Clears all items from the list
    pub fn clear_items(&mut self, cx: &mut Cx) {
        let list = self.portal_list(id!(list));
        if let Some(mut list) = list.borrow_mut() {
            list.set_item_range(cx, 0, 0);
        }

        self.selectable_widgets.clear();
        self.keyboard_focus_index = None;
        self.pointer_hover_index = None;
    }

    pub fn add_item(&mut self, cx: &mut Cx, widget_data: WidgetRef) {
        let current_items = self.selectable_widgets.len();
        let list = self.portal_list(id!(list));
        log!("Adding item to list {} with widget uid {:?}", current_items, list.widget_uid());

        // Create item using our UserListItem template
        let item = list.item(cx, current_items, live_id!(UserListItem));
        log!("Created list item with widget uid {:?}", item.widget_uid());

        // Set the text content
        item.label(id!(label)).set_text(&widget_data.label(id!(label)).text());
        item.label(id!(matrix_url)).set_text(&widget_data.label(id!(matrix_url)).text());

        // Add the item to our tracking list
        self.selectable_widgets.push(item.clone());

        // Update the list range
        if let Some(mut list) = list.borrow_mut() {
            list.set_item_range(cx, 0, self.selectable_widgets.len());
        }

        // Update keyboard focus if needed
        self.keyboard_focus_index = self.keyboard_focus_index.or(Some(0));

        // Make sure to redraw the popup
        self.view(id!(popup)).redraw(cx);
    }

    // // Adds a new item to the list
    // pub fn add_item(&mut self, cx: &mut Cx, widget_data: WidgetRef) {
    //     log!("command input bar add_item");
    //     let current_items = self.selectable_widgets.len();
    //     log!("command input bar current_items {}", current_items);
    //     let list = self.portal_list(id!(list));
    //     log!("command input bar get list {:?}", list.widget_uid());

    //     let item = list.item(cx, current_items, live_id!(user_list_item));

    //     log!("command input bar get item {:?}", item.widget_uid());

    //     item.label(id!(label)).set_text(&widget_data.label(id!(label)).text());
    //     item.label(id!(matrix_url)).set_text(&widget_data.label(id!(matrix_url)).text());

    //     self.selectable_widgets.push(item.clone());

    //     if let Some(mut list) = list.borrow_mut() {
    //         log!("command input bar get list {:?}", list.widget_uid());
    //         list.set_item_range(cx, 0, self.selectable_widgets.len());
    //     }

    //     self.keyboard_focus_index = self.keyboard_focus_index.or(Some(0));
    // }

    // Shows the popup menu
    fn show_popup(&mut self, cx: &mut Cx) {
        self.view(id!(popup)).set_visible(true);
        if let Some(mut list) = self.portal_list(id!(list)).borrow_mut() {
            list.set_item_range(cx, 0, self.selectable_widgets.len());
        }
        self.view(id!(popup)).redraw(cx);
    }

    // Hides the popup menu
    fn hide_popup(&mut self, cx: &mut Cx) {
        self.clear_items(cx);
        self.view(id!(popup)).set_visible(false);
    }

    // Handles selection of an item via keyboard
    fn select_focused_item(&mut self, cx: &mut Cx, scope: &mut Scope) {
        if let Some(idx) = self.keyboard_focus_index {
            if let Some(selected) = self.selectable_widgets.get(idx) {
                self.select_item(cx, scope, selected.clone());
            }
        }
    }

    // Handles selection of an item (via keyboard or mouse)
    fn select_item(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        self.last_selected_widget = selected;
        cx.widget_action(self.widget_uid(), &scope.path, InternalAction::ItemSelected);
        self.hide_popup(cx);
        self.is_text_input_focus_pending = true;
        self.try_remove_trigger_grapheme();
        self.redraw(cx);
    }

    // Removes trigger character after selection
    fn try_remove_trigger_grapheme(&mut self) {
        let head = self.text_input_ref()
            .borrow()
            .map_or(0, |p| p.get_cursor().head.index);

        if head == 0 { return; }

        let text = self.text();
        let trigger_pos = text[..head]
            .rfind('@')
            .filter(|&pos| !text[pos..head].contains(char::is_whitespace));

        if let Some(pos) = trigger_pos {
            let new_text = format!("{}{}", &text[..pos], &text[head..]);
            self.set_text(&new_text);
        }
    }

    // Updates visual highlights for keyboard/mouse focus
    fn update_highlights(&mut self, cx: &mut Cx) {
        for (idx, item) in self.selectable_widgets.iter().enumerate() {
            item.apply_over(cx, live! { show_bg: true, cursor: Hand });

            let bg_color = match (Some(idx) == self.keyboard_focus_index,
                                Some(idx) == self.pointer_hover_index) {
                (true, _) => self.keyboard_focus_color,
                (false, true) => self.pointer_hover_color,
                (false, false) => Vec4::all(0.0),
            };

            item.apply_over(cx, live! {
                draw_bg: { color: (bg_color) }
            });
        }
    }

    // Helper getters
    pub fn text_input_ref(&self) -> TextInputRef {
        self.text_input(id!(text_input))
    }

    fn trigger_grapheme(&self) -> Option<&str> {
        self.trigger.as_ref().and_then(|t| t.graphemes(true).next())
    }

    // Public methods for external components
    pub fn should_build_items(&self, actions: &Actions) -> bool {
        actions.iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == self.widget_uid())
            .any(|a| matches!(a.cast(), InternalAction::ShouldBuildItems))
    }

    pub fn item_selected(&self, actions: &Actions) -> Option<WidgetRef> {
        actions.iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == self.widget_uid())
            .find_map(|a| match a.cast() {
                InternalAction::ItemSelected => Some(self.last_selected_widget.clone()),
                _ => None
            })
    }
}

// Public interface for the component reference
impl CommandInputBarRef {
    pub fn should_build_items(&self, actions: &Actions) -> bool {
        self.borrow()
            .map_or(false, |inner| inner.should_build_items(actions))
    }

    pub fn clear_items(&mut self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_items(cx);
        }
    }

    pub fn add_item(&self, cx: &mut Cx, widget: WidgetRef) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.add_item(cx, widget);
        }
    }

    pub fn item_selected(&self, actions: &Actions) -> Option<WidgetRef> {
        self.borrow().and_then(|inner| inner.item_selected(actions))
    }

    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map_or(WidgetRef::empty().as_text_input(), |inner| {
                inner.text_input_ref()
            })
    }

    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.hide_popup(cx);
            inner.text_input_ref().set_text("");
        }
    }

    pub fn search_text(&self) -> String {
        self.borrow()
            .map_or(String::new(), |inner| inner.get_trigger_text())
    }
}

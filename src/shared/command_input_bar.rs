use makepad_widgets::*;
use unicode_segmentation::UnicodeSegmentation;

// Define the widget design template
live_design! {
    link widgets;
    use link::widgets::*;
    use link::theme::*;

    pub CommandInputBar = {{CommandInputBar}} {
        flow: Down,
        height: Fit,

        keyboard_focus_color: (THEME_COLOR_CTRL_HOVER),
        pointer_hover_color: (THEME_COLOR_CTRL_HOVER * 0.85),

        // The popup container that shows the list of items
        popup = <RoundedView> {
            flow: Down,
            height: Fit,
            visible: false,
            draw_bg: {
                color: #xffffff
            }

            // PortalList for scrollable list of items
            list = <PortalList> {
                // Setting height to a specific value to enable scrolling
                height: 200.0,
                width: Fill,
                flow: Down,
                spacing: 0.0,
                auto_tail: false
            }
        }

        // The persistent view that contains the text input
        persistent = <RoundedView> {
            flow: Down,
            height: Fit,
            top = <View> { height: Fit }
            center = <RoundedView> {
                height: Fit,
                left = <View> { width: Fit, height: Fit }
                text_input = <TextInput> {
                    width: Fill,
                    empty_message: "Type @ to trigger..."
                }
                right = <View> { width: Fit, height: Fit }
            }
            bottom = <View> { height: Fit }
        }
    }
}

// Internal actions used by the widget
#[derive(Debug, Copy, Clone, DefaultNone)]
enum InternalAction {
    ShouldBuildItems,
    ItemSelected,
    None,
}

/// `TextInput` wrapper with a popup list of options that appears when a
/// trigger character is typed.
///
/// Features:
/// - Trigger character to show popup (default '@')
/// - Keyboard navigation with arrow keys
/// - Mouse hover and click selection
/// - Scrollable list using PortalList
/// - Highlight current selection
#[derive(Widget, Live)]
pub struct CommandInputBar {
    #[deref]
    deref: View,

    /// The character that triggers the popup.
    /// If not set, popup can't be triggered by keyboard.
    #[live]
    pub trigger: Option<String>,

    /// Color for keyboard-focused item
    #[live]
    pub keyboard_focus_color: Vec4,

    /// Color for mouse-hovered item
    #[live]
    pub pointer_hover_color: Vec4,

    /// Text input focus state
    #[rust]
    is_text_input_focus_pending: bool,

    /// Currently keyboard-focused item index
    #[rust]
    keyboard_focus_index: Option<usize>,

    /// Currently mouse-hovered item index
    #[rust]
    pointer_hover_index: Option<usize>,

    /// List of selectable widgets
    #[rust]
    selectable_widgets: Vec<WidgetRef>,

    /// Last selected widget reference
    #[rust]
    last_selected_widget: WidgetRef,
}

impl Widget for CommandInputBar{
    fn set_text(&mut self, cx: &mut Cx, v: &str) {
        self.text_input_ref().set_text(cx, v);
    }

    fn text(&self) -> String {
        self.text_input_ref().text()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Handle main widget events
        self.deref.handle_event(cx, event, scope);

        // Handle text input events
        if let Event::TextInput(input_event) = event {
            if cx.has_key_focus(self.text_input_ref().area()) {
                self.on_text_inserted(cx, scope, &input_event.input);
            }
        }

        // Handle keyboard events
        if let Event::KeyDown(key_event) = event {
            if cx.has_key_focus(self.text_input_ref().area()) {
                let delta = match key_event.key_code {
                    KeyCode::ArrowDown => 1,
                    KeyCode::ArrowUp => -1,
                    KeyCode::Return => {
                        self.on_text_input_submit(cx, scope);
                        0
                    }
                    KeyCode::Escape => {
                        self.hide_popup(cx);
                        self.redraw(cx);
                        0
                    }
                    _ => 0
                };

                if delta != 0 {
                    self.on_keyboard_move(cx, delta);
                }
            }
        }

        // Handle action events
        if let Event::Actions(actions) = event {
            let mut selected_by_click = None;
            let mut should_redraw = false;

            // Handle mouse interactions with items
            for (idx, item) in self.selectable_widgets.iter().enumerate() {
                let item = item.as_view();

                // Handle clicks
                if item
                    .finger_down(actions)
                    .map(|fe| fe.tap_count == 1)
                    .unwrap_or(false)
                {
                    selected_by_click = Some((&*item).clone());
                }

                // Handle hover states
                if item.finger_hover_out(actions).is_some() && Some(idx) == self.pointer_hover_index
                {
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
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.update_highlights(cx);

        // Draw the portal list items
        if let Some(list) = self.portal_list(id!(list)).borrow_mut() {
            list.set_item_range(cx, 0, self.selectable_widgets.len());

            while let Some(item_id) = list.next_visible_item(cx) {
                if let Some(widget) = self.selectable_widgets.get(item_id) {
                    let item = list.item(cx, item_id, widget.widget_uid());

                    // Style the item
                    item.apply_over(cx, live! {
                        show_bg: true,
                        cursor: Hand,
                        width: Fill,
                        height: Fit,
                        padding: { left: 10.0, right: 10.0, top: 5.0, bottom: 5.0 }
                    });

                    // Apply highlight colors
                    if Some(item_id) == self.keyboard_focus_index {
                        item.apply_over(cx, live! {
                            draw_bg: { color: (self.keyboard_focus_color) }
                        });
                    } else if Some(item_id) == self.pointer_hover_index {
                        item.apply_over(cx, live! {
                            draw_bg: { color: (self.pointer_hover_color) }
                        });
                    }

                    // Draw the item
                    item.draw_all(cx, scope);
                }
            }
        }

        while !self.deref.draw_walk(cx, scope, walk).is_done() {}

        if self.is_text_input_focus_pending {
            self.is_text_input_focus_pending = false;
            self.text_input_ref().set_key_focus(cx);
        }

        DrawStep::done()
    }
}

impl CommandInputBar {
    fn on_text_inserted(&mut self, cx: &mut Cx, scope: &mut Scope, inserted: &str) {
        if graphemes(inserted).last() == self.trigger_grapheme() {
            self.show_popup(cx);
            cx.widget_action(
                self.widget_uid(),
                &scope.path,
                InternalAction::ShouldBuildItems,
            );
        }
    }

    fn on_text_input_submit(&mut self, cx: &mut Cx, scope: &mut Scope) {
        let Some(idx) = self.keyboard_focus_index else {
            return;
        };

        self.select_item(cx, scope, self.selectable_widgets[idx].clone());
    }

    fn select_item(&mut self, cx: &mut Cx, scope: &mut Scope, selected: WidgetRef) {
        self.last_selected_widget = selected;
        cx.widget_action(self.widget_uid(), &scope.path, InternalAction::ItemSelected);
        self.hide_popup(cx);
        self.is_text_input_focus_pending = true;
        self.try_remove_trigger_grapheme(cx);
        self.redraw(cx);
    }

    fn try_remove_trigger_grapheme(&mut self, cx: &mut Cx) {
        let head = get_head(&self.text_input_ref());
        if head == 0 {
            return;
        }

        let text = self.text();
        let Some((inserted_grapheme_pos, inserted_grapheme)) =
            inserted_grapheme_with_pos(&text, head)
        else {
            return;
        };

        if self.trigger_grapheme() == Some(inserted_grapheme) {
            let at_removed = graphemes_with_pos(&text)
                .filter_map(|(p, g)| {
                    if p == inserted_grapheme_pos {
                        None
                    } else {
                        Some(g)
                    }
                })
                .collect::<String>();

            self.set_text(cx, &at_removed);
        }
    }

    fn show_popup(&mut self, cx: &mut Cx) {
        self.view(id!(popup)).set_visible(cx, true);
        self.view(id!(popup)).redraw(cx);
    }

    fn hide_popup(&mut self, cx: &mut Cx) {
        self.clear_popup(cx);
        self.view(id!(popup)).set_visible(cx, false);
    }

    fn clear_popup(&mut self, _cx: &mut Cx) {
        self.selectable_widgets.clear();
        self.keyboard_focus_index = None;
        self.pointer_hover_index = None;
    }

    // Public API methods

    /// Clear all text and hide the popup going back to initial state.
    pub fn reset(&mut self, cx: &mut Cx) {
        self.clear_popup(cx);
        self.hide_popup(cx);
        self.text_input_ref().set_text(cx, "");
    }

    /// Clears the list of items.
    pub fn clear_items(&mut self) {
        self.selectable_widgets.clear();
        self.keyboard_focus_index = None;
        self.pointer_hover_index = None;
    }

    /// Add a selectable item to the list.
    pub fn add_item(&mut self, widget: WidgetRef) {
        self.selectable_widgets.push(widget);
        self.keyboard_focus_index = self.keyboard_focus_index.or(Some(0));
    }

    /// Add an unselectable item to the list (like headers or dividers).
    pub fn add_unselectable_item(&mut self, widget: WidgetRef) {
        // For unselectable items, we just want them in the visual list
        // but not in the selectable_widgets list used for navigation
        let portal_list = self.portal_list(id!(list));
        portal_list.add_item(widget);
    }

    /// Returns a reference to the inner `TextInput` widget.
    pub fn text_input_ref(&self) -> TextInputRef {
        self.text_input(id!(text_input))
    }

    /// Checks if any item has been selected in the given actions.
    pub fn item_selected(&self, actions: &Actions) -> Option<WidgetRef> {
        actions
            .iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == self.widget_uid())
            .find_map(|a| {
                if let InternalAction::ItemSelected = a.cast() {
                    Some(self.last_selected_widget.clone())
                } else {
                    None
                }
            })
    }

    /// Checks if items need to be rebuilt based on the given actions.
    pub fn should_build_items(&self, actions: &Actions) -> bool {
        actions
            .iter()
            .filter_map(|a| a.as_widget_action())
            .filter(|a| a.widget_uid == self.widget_uid())
            .any(|a| matches!(a.cast(), InternalAction::ShouldBuildItems))
    }

    /// Request focus for the text input.
    pub fn request_text_input_focus(&mut self) {
        self.is_text_input_focus_pending = true;
    }

    // Private helper methods

    fn trigger_grapheme(&self) -> Option<&str> {
        self.trigger.as_ref().and_then(|t| graphemes(t).next())
    }

    fn on_keyboard_move(&mut self, cx: &mut Cx, delta: i32) {
        let Some(idx) = self.keyboard_focus_index else {
            return;
        };

        let new_index = idx
            .saturating_add_signed(delta as isize)
            .clamp(0, self.selectable_widgets.len() - 1);

        if idx != new_index {
            self.keyboard_focus_index = Some(new_index);
        }

        self.redraw(cx);
    }

    fn update_highlights(&mut self, cx: &mut Cx) {
        for (idx, item) in self.selectable_widgets.iter().enumerate() {
            item.apply_over(cx, live! { show_bg: true, cursor: Hand });

            if Some(idx) == self.keyboard_focus_index {
                item.apply_over(
                    cx,
                    live! {
                        draw_bg: {
                            color: (self.keyboard_focus_color),
                        }
                    },
                );
            } else if Some(idx) == self.pointer_hover_index {
                item.apply_over(
                    cx,
                    live! {
                        draw_bg: {
                            color: (self.pointer_hover_color),
                        }
                    },
                );
            } else {
                item.apply_over(
                    cx,
                    live! {
                        draw_bg: {
                            color: (Vec4::all(0.)),
                        }
                    },
                );
            }
        }
    }
}

impl LiveHook for CommandInputBar {}

// Reference type implementation for the CommandInputBar widget
impl CommandInputBarRef {
    /// See [`CommandInputBar::should_build_items()`].
    pub fn should_build_items(&self, actions: &Actions) -> bool {
        self.borrow()
            .map_or(false, |inner| inner.should_build_items(actions))
    }

    /// See [`CommandInputBar::clear_items()`].
    pub fn clear_items(&mut self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_items();
        }
    }

    /// See [`CommandInputBar::add_item()`].
    pub fn add_item(&self, widget: WidgetRef) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.add_item(widget);
        }
    }

    /// See [`CommandInputBar::add_unselectable_item()`].
    pub fn add_unselectable_item(&self, widget: WidgetRef) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.add_unselectable_item(widget);
        }
    }

    /// See [`CommandInputBar::item_selected()`].
    pub fn item_selected(&self, actions: &Actions) -> Option<WidgetRef> {
        self.borrow().and_then(|inner| inner.item_selected(actions))
    }

    /// See [`CommandInputBar::text_input_ref()`].
    pub fn text_input_ref(&self) -> TextInputRef {
        self.borrow()
            .map_or(WidgetRef::empty().as_text_input(), |inner| {
                inner.text_input_ref()
            })
    }

    /// See [`CommandInputBar::reset()`].
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reset(cx);
        }
    }

    /// See [`CommandInputBar::request_text_input_focus()`].
    pub fn request_text_input_focus(&self) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.request_text_input_focus();
        }
    }
}

// Helper functions for text processing
fn graphemes(text: &str) -> impl DoubleEndedIterator<Item = &str> {
    text.graphemes(true)
}

fn graphemes_with_pos(text: &str) -> impl DoubleEndedIterator<Item = (usize, &str)> {
    text.grapheme_indices(true)
}

/// Find the grapheme at cursor position in text
fn inserted_grapheme_with_pos(text: &str, cursor_pos: usize) -> Option<(usize, &str)> {
    graphemes_with_pos(text).rfind(|(i, _)| *i < cursor_pos)
}

/// Get the current cursor head position from a TextInput
fn get_head(text_input: &TextInputRef) -> usize {
    text_input.borrow().map_or(0, |p| p.get_cursor().head.index)
}

// Error handling helper trait
trait ResultExt<T> {
    fn log_error(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> ResultExt<T> for Result<T, E> {
    fn log_error(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                error!("Error: {:?}", error);
                None
            }
        }
    }
}

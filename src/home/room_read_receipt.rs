use crate::shared::avatar::{AvatarRef, AvatarWidgetRefExt};
use crate::home::room_screen::RoomScreenTooltipActions;
use crate::utils::{self, human_readable_list};
use indexmap::IndexMap;
use makepad_widgets::*;
use matrix_sdk::ruma::{events::receipt::Receipt, EventId, OwnedUserId, RoomId};
use matrix_sdk_ui::timeline::EventTimelineItem;
use std::cmp;
const TOOLTIP_LENGTH: f64 = 150.0;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::avatar::*;
    use crate::shared::styles::*;

    pub AvatarRow = {{AvatarRow}} {
        avatar_template: <Avatar> {
            width: 15.0,
            height: 15.0,
            text_view = { 
                text = { 
                    draw_text: {
                        text_style: { font_size: 6.0 }
                    }
                }
            }
        }
        margin: {top: 12, right: 120, bottom: 3, left: 10},
        width: Fit,
        height: 30,
        plus_template: <Label> {
            draw_text: {
                color: #x0,
                text_style: <TITLE_TEXT>{ font_size: 11}
            }
            text: ""
        }
    }
}
/// The widget that displays a list of read receipts.
#[derive(Live, Widget, LiveHook)]
pub struct AvatarRow {
    #[redraw]
    #[live]
    draw_text: DrawText,
    #[deref]
    deref: View,
    #[walk]
    walk: Walk,
    /// The template for the avatars
    #[live]
    avatar_template: Option<LivePtr>,
    #[layout]
    layout: Layout,
    /// Label template for truncated number of people seen
    #[live]
    plus_template: Option<LivePtr>,
    /// A vector containing its avatarRef, its drawn status and username
    ///
    /// Storing the drawn status helps prevent unnecessary set avatar in the draw_walk function
    #[rust]
    buttons: Vec<(AvatarRef, bool, String)>,
    #[rust]
    label: Option<LabelRef>,
    /// The total number of receipts seen
    #[rust]
    total_num_seen: usize,
    /// The area of the widget
    #[redraw] 
    #[rust] 
    area: Area,
    /// The human readable usernames for tooltip
    #[rust]
    human_readable_usernames: String, 
}

impl Widget for AvatarRow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        if self.total_num_seen == 0 { return; }
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(finger_event) => {
                let tooltip_pos = DVec2 {
                    x: self.area.rect(cx).pos.x,
                    y: finger_event.abs.y
                };
                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverInReadReceipt{
                    tooltip_pos,
                    tooltip_text: format!("Seen by {:?} people\n{}", self.total_num_seen, self.human_readable_usernames), 
                    tooltip_width: TOOLTIP_LENGTH
                });
            }
            Hit::FingerHoverOut(_) => {
                cx.widget_action(uid, &scope.path, RoomScreenTooltipActions::HoverOut);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        for (avatar_ref, _, _) in self.buttons.iter_mut() {
            let _ = avatar_ref.draw(cx, scope);
        }
        if self.total_num_seen > utils::MAX_VISIBLE_NUMBER_OF_ITEMS {
            if let Some(label) = &mut self.label {
                label.set_text(&format!(" + {:?}", self.total_num_seen - utils::MAX_VISIBLE_NUMBER_OF_ITEMS));
                let _ = label.draw(cx, scope);
            }
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
impl AvatarRow {
    
    /// Set a row of Avatars based on receipt index map
    ///
    /// Given a sequence of receipts, this will set each of the first MAX_VISIBLE_AVATARS_IN_READ_RECEIPT_ROW
    /// avatars in this row to the corresponding user's avatar, and
    /// display the given number of total receipts as a label. The
    /// index map is expected to yield tuples of (user_id, receipt),
    /// where the receipt is ignored.
    fn set_avatar_row(
        &mut self,
        cx: &mut Cx,
        room_id: &RoomId,
        event_id: Option<&EventId>,
        receipts_map: &IndexMap<OwnedUserId, Receipt>) {
        if receipts_map.len() != self.buttons.len() {
            self.buttons.clear();
            for _ in 0..cmp::min(utils::MAX_VISIBLE_NUMBER_OF_ITEMS, receipts_map.len()) {
                self.buttons.push((WidgetRef::new_from_ptr(cx, self.avatar_template).as_avatar(), false, String::new()));
            }
        }
        self.total_num_seen = receipts_map.len();
        self.label = Some(WidgetRef::new_from_ptr(cx, self.plus_template).as_label());
        for ((avatar_ref, drawn, username_ref), (user_id, _)) in self.buttons.iter_mut().zip(receipts_map.iter().rev()) {
            if !*drawn {
                let (username, drawn_status) = avatar_ref.set_avatar_and_get_username(cx, room_id, user_id, None, event_id); 
                *drawn = drawn_status;
                *username_ref = username;
            }
        }
        let mut username_arr: Vec<String> = self.buttons.iter().map(|(_, _, username)| username.clone()).collect();
        for _ in username_arr.len()..receipts_map.len() {
            username_arr.push(String::new());
        }
        self.human_readable_usernames = human_readable_list(&username_arr);
    }
}
impl AvatarRowRef {
    /// Handles hover in action
    pub fn hover_in(&self, actions: &Actions) -> RoomScreenTooltipActions {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            item.cast()
        } else {
            RoomScreenTooltipActions::None
        }
    }
    /// Returns true if the action is a hover out
    pub fn hover_out(&self, actions: &Actions) -> bool {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            matches!(item.cast(), RoomScreenTooltipActions::HoverOut)
        } else {
            false
        }
    }
    /// Get the total number of people seen 
    pub fn total_num_seen(&self) -> usize {
        if let Some(ref mut inner) = self.borrow() {
            inner.total_num_seen
        } else {
            0
        }
    }
    /// See [`AvatarRow::set_avatar_row()`].
    pub fn set_avatar_row(&mut self, cx: &mut Cx, room_id: &RoomId, event_id: Option<&EventId>, receipts_map: &IndexMap<OwnedUserId, Receipt>) {
        if let Some(ref mut inner) = self.borrow_mut() {
            inner.set_avatar_row(cx, room_id, event_id, receipts_map);
        }
    }
}

/// Populate the read receipts avatar row in a message item
/// 
/// Given a reference to item widget (typically a MessageEventMarker), a Cx2d, a
/// room ID, and an EventTimelineItem, this will populate the avatar
/// row of the item with the read receipts of the event.
///
pub fn populate_read_receipts(item: &WidgetRef, cx: &mut Cx, room_id: &RoomId, event_tl_item: &EventTimelineItem) {
    item.avatar_row(id!(avatar_row)).set_avatar_row(cx, room_id, event_tl_item.event_id(), event_tl_item.read_receipts());
}

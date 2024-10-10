use super::*;

#[derive(Clone, Debug)]
pub struct CollapsedStateEvents {
    start_index: usize,
    end_index: usize,
}

impl RoomScreen{

    pub(crate) fn draw_timeline(cx: &mut Cx2d, mut list_ref: &mut PortalList, tl_state: &mut TimelineUiState) {
        let room_id = &tl_state.room_id;
        let tl_items = &tl_state.items;

        // Set the portal list's range based on the number of timeline items.
        let last_item_id = tl_items.len();

        let list = list_ref.deref_mut();
        list.set_item_range(cx, 0, last_item_id);

        while let Some(item_id) = list.next_visible_item(cx) {
            let item = {
                let tl_idx = item_id as usize;
                let Some(timeline_item) = tl_items.get(tl_idx) else {
                    // This shouldn't happen (unless the timeline gets corrupted or some other weird error),
                    // but we can always safely fill the item with an empty widget that takes up no space.
                    list.item(cx, item_id, live_id!(Empty)).unwrap();
                    continue;
                };

                // Determine whether this item's content and profile have been drawn since the last update.
                // Pass this state to each of the `populate_*` functions so they can attempt to re-use
                // an item in the timeline's portallist that was previously populated, if one exists.
                let item_drawn_status = ItemDrawnStatus {
                    content_drawn: tl_state.content_drawn_since_last_update.contains(&tl_idx),
                    profile_drawn: tl_state.profile_drawn_since_last_update.contains(&tl_idx),
                };

                let (item, item_new_draw_status) = match timeline_item.kind() {
                    TimelineItemKind::Event(event_tl_item) => match event_tl_item.content() {
                        TimelineItemContent::Message(message) => {
                            let prev_event = tl_items.get(tl_idx.saturating_sub(1));
                            populate_message_view(
                                cx,
                                list,
                                item_id,
                                room_id,
                                event_tl_item,
                                message,
                                prev_event,
                                &mut tl_state.media_cache,
                                item_drawn_status,
                            )
                        }
                        TimelineItemContent::RedactedMessage => populate_small_state_event(
                            cx,
                            list,
                            item_id,
                            room_id,
                            event_tl_item,
                            &RedactedMessageEventMarker,
                            item_drawn_status,
                        ),
                        TimelineItemContent::MembershipChange(membership_change) => populate_small_state_event(
                            cx,
                            list,
                            item_id,
                            room_id,
                            event_tl_item,
                            membership_change,
                            item_drawn_status,
                        ),
                        TimelineItemContent::ProfileChange(profile_change) => populate_small_state_event(
                            cx,
                            list,
                            item_id,
                            room_id,
                            event_tl_item,
                            profile_change,
                            item_drawn_status,
                        ),
                        TimelineItemContent::OtherState(other) => populate_small_state_event(
                            cx,
                            list,
                            item_id,
                            room_id,
                            event_tl_item,
                            other,
                            item_drawn_status,
                        ),
                        unhandled => {
                            let item = list.item(cx, item_id, live_id!(SmallStateEvent)).unwrap();
                            item.label(id!(content)).set_text(&format!("[TODO] {:?}", unhandled));
                            (item, ItemDrawnStatus::both_drawn())
                        }
                    }
                    TimelineItemKind::Virtual(VirtualTimelineItem::DayDivider(millis)) => {
                        let item = list.item(cx, item_id, live_id!(DayDivider)).unwrap();
                        let text = unix_time_millis_to_datetime(&millis)
                            // format the time as a shortened date (Sat, Sept 5, 2021)
                            .map(|dt| format!("{}", dt.date_naive().format("%a %b %-d, %Y")))
                            .unwrap_or_else(|| format!("{:?}", millis));
                        item.label(id!(date)).set_text(&text);
                        (item, ItemDrawnStatus::both_drawn())
                    }
                    TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => {
                        let item = list.item(cx, item_id, live_id!(ReadMarker)).unwrap();
                        (item, ItemDrawnStatus::both_drawn())
                    }
                };

                // Now that we've drawn the item, add its index to the set of drawn items.
                if item_new_draw_status.content_drawn {
                    tl_state.content_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                }
                if item_new_draw_status.profile_drawn {
                    tl_state.profile_drawn_since_last_update.insert(tl_idx .. tl_idx + 1);
                }
                item
            };
            item.draw_all(cx, &mut Scope::empty());
        }

    }


    // pub(crate) fn detect_and_collapse_state_events(&mut self, items: &[Arc<TimelineItem>]) {
    //     let mut current_group: Option<CollapsedStateEvents> = None;
    //     let mut collapsed_groups = Vec::new();

    //     for (index, item) in items.iter().enumerate() {
    //         if Self::is_small_state_event(item) {
    //             if let Some(group) = &mut current_group {
    //                 group.end_index = index;
    //             } else {
    //                 current_group = Some(CollapsedStateEvents {
    //                     start_index: index,
    //                     end_index: index,
    //                 });
    //             }
    //         } else {
    //             if let Some(group) = current_group.take() {
    //                 if group.end_index - group.start_index >= 1 { // 至少两个连续的 small state events
    //                     collapsed_groups.push(group);
    //                 }
    //             }
    //         }
    //     }

    //     if let Some(group) = current_group {
    //         if group.end_index - group.start_index >= 1 {
    //             collapsed_groups.push(group);
    //         }
    //     }

    //     if let Some(tl_state) = &mut self.tl_state {
    //         tl_state.collapsed_state_events = collapsed_groups;
    //     }
    // }

    // pub(crate) fn is_small_state_event(item: &TimelineItem) -> bool {
    //     matches!(item.kind(),
    //         TimelineItemKind::Event(event) if matches!(event.content(),
    //             TimelineItemContent::MembershipChange(_) |
    //             TimelineItemContent::ProfileChange(_) |
    //             TimelineItemContent::OtherState(_)
    //         )
    //     )
    // }

    // pub(crate) fn draw_collapsed_state_events(&mut self, cx: &mut Cx2d, list: &mut PortalList, item_id: usize, group: &CollapsedStateEvents) {
    //     let item = list.item(cx, item_id, live_id!(CollapsibleView)).unwrap();
    //     let collapsible = item.as_collapsible_view().unwrap();

    //     let summary = self.generate_summary(group);
    //     collapsible.set_summary(cx, &summary);

    //     let button_text = if collapsible.is_expanded() { "Collapse" } else { "Expand" };
    //     collapsible.set_toggle_text(cx, button_text);

    //     if collapsible.is_expanded() {
    //         let content = collapsible.content_view();
    //         for i in group.start_index..=group.end_index {
    //             if let Some(event) = self.tl_state.as_ref().unwrap().items.get(i) {
    //                 self.draw_single_event(cx, &content, i, event);
    //             }
    //         }
    //     }
    // }

    // pub(crate) fn generate_summary(&self, group: &CollapsedStateEvents) -> String {
    //     let event_count = group.end_index - group.start_index + 1;
    //     let mut summary = format!("{} events: ", event_count);

    //     let mut event_types = std::collections::HashMap::new();
    //     for i in group.start_index..=group.end_index {
    //         if let Some(event) = self.tl_state.as_ref().unwrap().items.get(i) {
    //             let event_type = self.get_event_type(event);
    //             *event_types.entry(event_type).or_insert(0) += 1;
    //         }
    //     }

    //     for (event_type, count) in event_types {
    //         summary += &format!("{} {}, ", count, event_type);
    //     }
    //     summary.trim_end_matches(", ").to_string()
    // }

    // pub(crate) fn get_event_type(&self, event: &TimelineItem) -> &'static str {
    //     match event.kind() {
    //         TimelineItemKind::Event(event) => match event.content() {
    //             TimelineItemContent::MembershipChange(_) => "joined",
    //             TimelineItemContent::ProfileChange(pc) => {
    //                 if pc.displayname_changed() { "changed name" }
    //                 else if pc.avatar_url_changed() { "changed avatar" }
    //                 else { "changed profile" }
    //             },
    //             TimelineItemContent::OtherState(_) => "made changes",
    //             _ => "other events",
    //         },
    //         _ => "other events",
    //     }
    // }

    // pub(crate) fn find_collapsed_group(&self, item_id: usize) -> Option<&CollapsedStateEvents> {
    //     self.tl_state.as_ref()?.collapsed_state_events.iter()
    //         .find(|group| group.start_index <= item_id && item_id <= group.end_index)
    // }

}

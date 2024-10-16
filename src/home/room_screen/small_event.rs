use super::*;
use std::collections::VecDeque;


live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;

    CollapsibleEventGroup = <View> {
        width: Fill, height: Fit
        flow: Down

        collapsible_content = <GCollapse> {
            opened: false
            header = {
                <View> {
                    width: Fill, height: Fit
                    flow: Right
                    spacing: 10.0
                    padding: 5.0

                    summary_text = <Label> {
                        width: Fill
                        text: "Collapsed events"
                        draw_text: {
                            text_style: <REGULAR_TEXT>{font_size: 14},
                            color: #000000
                        }
                    }

                    toggle_icon = <Icon> {
                        width: 20
                        height: 20
                        // TODO: Add appropriate icon for expand/collapse
                    }
                }
            }
            body = {
                <View> {
                    width: Fill, height: Fit
                    flow: Down
                    spacing: 5.0
                    padding: {left: 20.0, top: 5.0, right: 5.0, bottom: 5.0}

                    events_list = <PortalList> {
                        width: Fill
                        height: Fit
                        flow: Down
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CollapsibleEvent {
    event: Arc<TimelineItem>,
    is_collapsible: bool,
}

#[derive(Default)]
pub struct CollapsedEventsGroup {
    events: VecDeque<CollapsibleEvent>,
}


impl CollapsedEventsGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_event(&mut self, event: CollapsibleEvent) {
        self.events.push_back(event);
    }

    pub fn is_collapsible(&self) -> bool {
        self.events.len() >= 2 && self.events.iter().all(|e| e.is_collapsible)
    }

    pub fn summarize(&self) -> String {
        let mut event_counts: HashMap<&str, usize> = HashMap::new();

        for event in &self.events {
            let event_type = match event.event.kind() {
                TimelineItemKind::Event(event_tl_item) => match event_tl_item.content() {
                    TimelineItemContent::MembershipChange(_) => "membership change",
                    TimelineItemContent::ProfileChange(_) => "profile change",
                    TimelineItemContent::OtherState(_) => "other state change",
                    _ => "other event",
                },
                _ => "other event",
            };
            *event_counts.entry(event_type).or_insert(0) += 1;
        }

        let total_events = self.events.len();
        let mut event_summaries: Vec<String> = event_counts
            .iter()
            .map(|(event_type, count)| format!("{} {}", count, event_type))
            .collect();

        if event_summaries.len() > 2 {
            let others_count: usize = event_summaries[2..].iter()
                .map(|s| s.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0))
                .sum();
            event_summaries.truncate(2);
            event_summaries.push(format!("{} other events", others_count));
        }

        let summary = event_summaries.join(", ");
        format!("{} events: {}", total_events, summary)
    }
}

impl RoomScreen {

    pub fn process_timeline_events(&mut self, cx: &mut Cx) {
        let Some(tl) = self.tl_state.as_mut() else { return };
        let mut collapsed_groups: Vec<CollapsedEventsGroup> = Vec::new();
        let mut current_group = CollapsedEventsGroup::new();

        for event in tl.items.iter() {
            let is_collapsible = Self::is_event_collapsible(event);
            let collapsible_event = CollapsibleEvent {
                event: event.clone(),
                is_collapsible,
            };

            if is_collapsible {
                current_group.add_event(collapsible_event);
            } else {
                if current_group.is_collapsible() {
                    collapsed_groups.push(std::mem::take(&mut current_group));
                } else if !current_group.events.is_empty() {
                    collapsed_groups.extend(current_group.events.drain(..).map(|e| {
                        let mut group = CollapsedEventsGroup::new();
                        group.add_event(e);
                        group
                    }));
                }
                let mut new_group = CollapsedEventsGroup::new();
                new_group.add_event(collapsible_event);
                collapsed_groups.push(new_group);
            }
        }

        if current_group.is_collapsible() {
            collapsed_groups.push(current_group);
        } else if !current_group.events.is_empty() {
            collapsed_groups.extend(current_group.events.drain(..).map(|e| {
                let mut group = CollapsedEventsGroup::new();
                group.add_event(e);
                group
            }));
        }

        tl.collapsed_groups = collapsed_groups;
        self.redraw(cx);
    }

    pub fn is_event_collapsible(event: &Arc<TimelineItem>) -> bool {
        matches!(
            event.kind(),
            TimelineItemKind::Event(EventTimelineItem { content: TimelineItemContent::MembershipChange(_) | TimelineItemContent::ProfileChange(_) | TimelineItemContent::OtherState(_), .. })
        )
    }

    pub fn draw_event_group(&mut self, cx: &mut Cx2d, list: &mut PortalList, group: &CollapsedEventsGroup, item_id: usize) -> WidgetRef {
        if group.is_collapsible() {
            let item = list.item(cx, item_id, live_id!(CollapsibleEventGroup));
            let gcollapse = item.gcollapse(id!(collapsible_content));

            // Set the summary text
            let summary_text = group.summarize();
            gcollapse.view(id!(header)).label(id!(summary_text)).set_text(&summary_text);

            // Populate the body with individual events
            if let Some(body) = gcollapse.body() {
                let events_list = body.portal_list(id!(events_list));
                for (index, event) in group.events.iter().enumerate() {
                    let event_item = self.draw_single_event(cx, events_list, &event.event, index);
                    events_list.add_item(event_item);
                }
            }

            item
        } else {
            // For non-collapsible groups (single events), draw them directly
            let event = &group.events[0].event;
            self.draw_single_event(cx, list, event, item_id)
        }
    }

    pub fn draw_single_event(&mut self, cx: &mut Cx2d, list: &mut PortalList, event: &Arc<TimelineItem>, item_id: usize) -> WidgetRef {
        match event.kind() {
            TimelineItemKind::Event(event_tl_item) => match event_tl_item.content() {
                TimelineItemContent::Message(message) => {
                    // Implement message drawing logic
                    list.item(cx, item_id, live_id!(Message))
                }
                _ => {
                    // Implement small state event drawing logic
                    list.item(cx, item_id, live_id!(SmallStateEvent))
                }
            },
            TimelineItemKind::Virtual(_) => {
                // Handle virtual items (day dividers, read markers, etc.)
                list.item(cx, item_id, live_id!(Empty))
            }
        }
    }
}

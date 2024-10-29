use super::*;

use super::event::*;
use types::*;

use crate::{
    animatie_fn, event_option, ref_event_option, set_event,
    shared::shader::{draw_view::DrawGView, manual::Position4},
    widget_area,
};

use self::utils::{set_cursor, BoolToF32};

live_design! {
    import makepad_draw::shader::std::*;
    GLOBAL_DURATION = 0.25;
    GCollapseBase = {{GCollapse}}{
        animator: {
            open = {
                default: off
                off = {
                    from: {all: Forward {duration: (GLOBAL_DURATION)}}
                    ease: ExpDecay {d1: 0.96, d2: 0.97}
                    redraw: true
                    apply: {
                        fold: [{time: 0.0, value: 1.0}, {time: 1.0, value: 0.0}]
                    }
                }
                on = {
                    from: {all: Forward {duration: (GLOBAL_DURATION)}}
                    ease: ExpDecay {d1: 0.98, d2: 0.95}
                    redraw: true
                    apply: {
                        fold: [{time: 0.0, value: 0.0}, {time: 1.0, value: 1.0}]
                    }
                }
            }
        }
    }
}

#[derive(Live, LiveHook,Widget)]
pub struct GCollapse {
    #[live]
    #[redraw]
    #[find]
    pub header: WidgetRef,
    #[live]
    #[redraw]
    #[find]
    pub body: WidgetRef,
    #[redraw]
    #[live]
    pub draw_collapse: DrawGView,
    #[walk]
    pub walk: Walk,
    #[layout]
    pub layout: Layout,
    #[rust]
    pub rect_size: f64,
    #[rust]
    pub area: Area,
    #[live(false)]
    pub opened: bool,
    #[live]
    fold: f64,
    #[rust]
    pub draw_state: DrawStateWrap<DrawCollapseState>,
    #[live(Some(MouseCursor::Hand))]
    pub cursor: Option<MouseCursor>,
    #[live(true)]
    pub grab_key_focus: bool,
    // visible -------------------
    #[live(true)]
    pub visible: bool,
    // animator -----------------
    #[live(false)]
    pub animation_open: bool,
    #[animator]
    animator: Animator,
    // use animation counter to prevent multiple animations
    #[rust(true)]
    animation_counter: bool,
    #[live]
    pub position: Position4,
}

impl Widget for GCollapse {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        log!("GCollapse draw_walk: visible={}, opened={}, fold={}", self.visible, self.opened, self.fold);
        if !self.visible {
            return DrawStep::done();
        }

        // 获取动画状态和折叠值
        if let Some(state) = self.animator.state.as_ref() {
            if let Some(LiveValue::Float64(fold)) = state.child_value_by_path(0, &[live_id!(tracks).as_field(), live_id!(open).as_field(), live_id!(fold).as_field()]) {
                self.fold = *fold;
            }
        } else {
            // 如果没有动画状态，使用 opened 状态设置初始 fold 值
            self.fold = self.opened.to_f32() as f64;
        }

        let body_walk = self.body.walk(cx);
        log!("Body walk: {:?}", body_walk);

        let header_walk = self.header.walk(cx);



        let (flow, steps) = match self.position {
            Position4::Left => (
                Flow::Right,
                [DrawCollapseState::DrawBody, DrawCollapseState::DrawHeader],
            ),
            Position4::Right => (
                Flow::Right,
                [DrawCollapseState::DrawHeader, DrawCollapseState::DrawBody],
            ),
            Position4::Top => (
                Flow::Down,
                [DrawCollapseState::DrawBody, DrawCollapseState::DrawHeader],
            ),
            Position4::Bottom => (
                Flow::Down,
                [DrawCollapseState::DrawHeader, DrawCollapseState::DrawBody],
            ),
        };

        self.layout.flow = flow;
        if self.draw_state.begin(cx, steps[0]) {
            cx.begin_turtle(walk, self.layout);
        }

        for (index, _) in steps.iter().enumerate() {
            let _ = self.draw_state.get().map(|state| match state {
                DrawCollapseState::DrawHeader => {
                    let _ = self.header.draw_walk(cx, scope, header_walk);
                    // check is the first step
                    if index == 0 {
                        cx.begin_turtle(
                            body_walk,
                            Layout::flow_down()
                                .with_scroll(dvec2(0.0, self.rect_size * (1.0 - self.fold))),
                        );
                        self.draw_state.set(steps[1]);
                    } else {
                        match self.position {
                            Position4::Left | Position4::Right => {
                                self.rect_size = cx.turtle().used().x;
                            }
                            Position4::Top | Position4::Bottom => {
                                self.rect_size = cx.turtle().used().y;
                            }
                        }
                        cx.end_turtle();
                        cx.end_turtle_with_area(&mut self.area);
                        self.draw_state.end();
                    }
                }
                DrawCollapseState::DrawBody => {
                    log!("Body walk height: {:?}", body_walk.height);

                    let mut body_height = 0.0;

                    cx.begin_turtle(body_walk, Layout::flow_down());
                    let _ = self.body.draw_walk(cx, scope, body_walk);
                    body_height = cx.turtle().used().y;
                    let body_rect = cx.end_turtle();
                    log!("body_rect: {:?}", body_rect);

                    let body_height = if body_height.is_nan() { 100.0 } else { body_height };
                    log!("GCollapse DrawBody: body_height={}, fold={}", body_height, self.fold);
                    let animated_height = body_height * self.fold;
                    log!("GCollapse DrawBody: animated_height={}", animated_height);
                    log!("GCollapse DrawBody: body_height={}, animated_height={}", body_height, animated_height);
                    if animated_height > 0.0 {
                        let body_walk = Walk {
                            width: Size::Fill,
                            height: Size::Fixed(animated_height),
                            ..Walk::default()
                        };

                        cx.begin_turtle(body_walk, Layout::flow_down());
                        let _ = self.body.draw_walk(cx, scope, Walk::default());
                        cx.end_turtle();
                    }
                    // 更新整个组件的高度
                    let total_height = body_height + animated_height;
                    cx.turtle_mut().update_height_max(0.0, total_height);
                    log!("GCollapse total height set to: {}", total_height);


                    // 确保父容器知道这个新的高度
                    let current_used = cx.turtle().used();
                    cx.turtle_mut().set_used(current_used.x, total_height);

                    // check is the last step
                    if index == 1 {
                        match self.position {
                            Position4::Left | Position4::Right => {
                                self.rect_size = cx.turtle().used().x;
                            }
                            Position4::Top | Position4::Bottom => {
                                self.rect_size = cx.turtle().used().y;
                            }
                        }
                        cx.end_turtle();
                        cx.end_turtle_with_area(&mut self.area);
                        self.draw_state.end();
                    } else {
                        cx.begin_turtle(header_walk, Layout::flow_down());
                        self.draw_state.set(steps[1]);
                    }
                }
            });
        }
        DrawStep::done()
    }
    fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        scope: &mut Scope,
        sweep_area: Area,
    ) {
        let uid = self.widget_uid();
        if !self.animation_open && self.animation_counter {
            if self.animator_handle_event(cx, event).must_redraw() {
                if self.animator.is_track_animating(cx, id!(open)) {
                    self.area.redraw(cx);
                    self.animation_counter = !self.animation_counter;
                }
            }
        }

        match event.hits(cx, self.area_header()) {
            Hit::FingerDown(_) => {
                if self.grab_key_focus {
                    cx.set_key_focus(sweep_area);
                }
            }
            Hit::FingerHoverIn(f_in) => {
                let _ = set_cursor(cx, self.cursor.as_ref());
                cx.widget_action(uid, &scope.path, GCollapseEvent::Hover(f_in.clone()));
            }
            Hit::FingerHoverOut(_) => {
                let _ = set_cursor(cx, Some(&MouseCursor::Arrow));
            }
            Hit::FingerUp(f_up) => {
                self.opened = !self.opened;
                self.fold = self.opened.to_f32() as f64;

                if self.opened {
                    self.animator_play(cx, id!(open.on));
                    cx.widget_action(uid, &scope.path, GCollapseEvent::Opened(f_up.clone()));
                } else {
                    self.animator_play(cx, id!(open.off));
                    cx.widget_action(uid, &scope.path, GCollapseEvent::Closed(f_up.clone()));
                }
                self.animation_counter = !self.animation_counter;
            }
            _ => {}
        }

        if self.opened {
            self.body.handle_event(cx, event, scope);
        }
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();

        if self.animator_handle_event(cx, event).is_animating() {
            log!("GCollapse animating, requesting redraw");
            cx.redraw_all();
        }
        if !self.animation_open && self.animation_counter {
            if self.animator_handle_event(cx, event).must_redraw() {
                if self.animator.is_track_animating(cx, id!(open)) {
                    self.area.redraw(cx);
                    self.animation_counter = !self.animation_counter;
                }
            }
        }

        match event.hits(cx, self.area_header()) {
            Hit::FingerDown(_) => {
                if self.grab_key_focus {
                    cx.set_key_focus(self.area());
                }
            }
            Hit::FingerHoverIn(f_in) => {
                let _ = set_cursor(cx, self.cursor.as_ref());
                cx.widget_action(uid, &scope.path, GCollapseEvent::Hover(f_in.clone()));
            }
            Hit::FingerHoverOut(_) => {
                let _ = set_cursor(cx, Some(&MouseCursor::Arrow));
            }
            Hit::FingerUp(f_up) => {
                self.opened = !self.opened;
                log!("GCollapse toggled: opened={}", self.opened);
                if self.opened {
                    self.animator_play(cx, id!(open.on));
                    cx.widget_action(uid, &scope.path, GCollapseEvent::Opened(f_up.clone()));
                } else {
                    self.animator_play(cx, id!(open.off));
                    cx.widget_action(uid, &scope.path, GCollapseEvent::Closed(f_up.clone()));
                }
                self.animation_counter = true;
                cx.redraw_all();
            }
            _ => {}
        }

        if self.opened {
            self.body.handle_event(cx, event, scope);
        }
    }
}

impl GCollapse {
    widget_area! {
        area, area,
        area_header, header,
        area_body, body
    }
    event_option! {
        opened: GCollapseEvent::Opened => FingerUpEvent,
        closed: GCollapseEvent::Closed => FingerUpEvent,
        hover: GCollapseEvent::Hover => FingerHoverEvent
    }
    pub fn animate_open_on(&mut self, cx: &mut Cx) -> () {
        log!("GCollapse animate_open_on");
        self.opened = true;
        self.fold = 1.0;
        self.animator_play(cx, id!(open.on));
        self.animation_counter = true;
        self.redraw(cx);
    }
    pub fn animate_open_off(&mut self, cx: &mut Cx) -> () {
        log!("GCollapse animate_open_off");
        self.opened = false;
        self.fold = 0.0;
        self.animator_play(cx, id!(open.off));
        self.animation_counter = true;
        self.redraw(cx);
    }
}

impl GCollapseRef {
    ref_event_option! {
        opened => FingerUpEvent,
        closed => FingerUpEvent,
        hover => FingerHoverEvent
    }
    animatie_fn! {
        animate_open_on,
        animate_open_off
    }
}

impl GCollapseSet {
    set_event! {
        opened => FingerUpEvent,
        closed => FingerUpEvent,
        hover => FingerHoverEvent
    }
}

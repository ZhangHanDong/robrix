use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

    pub InputBar = {{InputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        align: {y: 0.5}
        padding: 10.
        show_bg: true
        draw_bg: {
            color: (COLOR_PRIMARY)
        }

        // 位置按钮配置
        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        // 消息输入框配置
        message_input = <TextInput> {
            width: Fill,
            height: Fit,
            margin: 0
            align: {y: 0.5}
            empty_message: "Write a message (in Markdown) ..."

            // 背景绘制配置
            draw_bg: {
                color: (COLOR_PRIMARY)
                instance radius: 2.0
                instance border_width: 0.8
                instance border_color: #D0D5DD
                instance inset: vec4(0.0, 0.0, 0.0, 0.0)

                fn get_color(self) -> vec4 {
                    return self.color
                }

                fn get_border_color(self) -> vec4 {
                    return self.border_color
                }

                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size)
                    sdf.box(
                        self.inset.x + self.border_width,
                        self.inset.y + self.border_width,
                        self.rect_size.x - (self.inset.x + self.inset.z + self.border_width * 2.0),
                        self.rect_size.y - (self.inset.y + self.inset.w + self.border_width * 2.0),
                        max(1.0, self.radius)
                    )
                    sdf.fill_keep(self.get_color())
                    if self.border_width > 0.0 {
                        sdf.stroke(self.get_border_color(), self.border_width)
                    }
                    return sdf.result;
                }
            }

            // 文本绘制配置
            draw_text: {
                color: (MESSAGE_TEXT_COLOR)
                text_style: <MESSAGE_TEXT_STYLE>{}

                fn get_color(self) -> vec4 {
                    return mix(
                        self.color,
                        #B,
                        self.is_empty
                    )
                }
            }

            // 光标渲染配置 - 使用 shader 实现闪烁
            draw_cursor: {
                instance focus: 0.0
                uniform border_radius: 0.5
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#0f0, #0b0, self.focus));
                    return sdf.result
                }
            }

            // 选择区域绘制配置
            draw_selection: {
                instance hover: 0.0
                instance focus: 0.0
                uniform border_radius: 2.0
                fn pixel(self) -> vec4 {
                    let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                    sdf.box(
                        0.,
                        0.,
                        self.rect_size.x,
                        self.rect_size.y,
                        self.border_radius
                    )
                    sdf.fill(mix(#dfffd6, #bfffb0, self.focus));
                    return sdf.result
                }
            }
        }

        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

// 定义组件可以触发的动作
#[derive(Clone, Debug)]
pub enum InputBarAction {
    LocationButtonClicked,
    MessageChanged(String),
}

// 组件主结构 - 移除了动画相关字段
#[derive(Live, Widget)]
pub struct InputBar {
    #[deref]
    view: View,
    #[rust(false)]
    has_focus: bool,
}

impl LiveHook for InputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        self.has_focus = true;

        // 确保消息输入框完全初始化并获得焦点
        let message_input = self.text_input(id!(message_input));
        cx.set_key_focus(message_input.area());
        cx.show_text_ime(message_input.area(), DVec2::default());

        // 请求完整重绘以确保显示正确
        cx.redraw_all();
    }
}

impl Widget for InputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let ret = self.view.draw_walk(cx, scope, walk);

        // 确保输入法显示在正确位置
        if self.has_focus {
            let text_input = self.text_input(id!(message_input));
            cx.show_text_ime(text_input.area(), DVec2::default());
        }

        ret
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let message_input = self.text_input(id!(message_input));

        match event {
            Event::Actions(actions) => {
                for action in actions {
                    if let Some(widget_action) = action.as_widget_action() {
                        if widget_action.widget_uid == message_input.widget_uid() {
                            match widget_action.cast::<TextInputAction>() {
                                TextInputAction::KeyFocus => {
                                    self.has_focus = true;
                                    cx.set_key_focus(message_input.area());
                                    cx.show_text_ime(message_input.area(), DVec2::default());
                                    self.redraw(cx);
                                }
                                TextInputAction::KeyFocusLost => {
                                    self.has_focus = false;
                                    cx.hide_text_ime();
                                    self.redraw(cx);
                                }
                                TextInputAction::Change(new_text) => {
                                    cx.widget_action(widget_uid, &scope.path,
                                        InputBarAction::MessageChanged(new_text));
                                    cx.show_text_ime(message_input.area(), DVec2::default());
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if self.button(id!(location_button)).clicked(actions) {
                    cx.widget_action(widget_uid, &scope.path,
                        InputBarAction::LocationButtonClicked);
                }
            }
            _ => {}
        }

        self.view.handle_event(cx, event, scope);
    }
}

// 组件方法实现
impl InputBar {
    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.text_input(id!(message_input)).set_text_and_redraw(cx, text);
    }

    pub fn set_key_focus(&mut self, cx: &mut Cx) {
        self.text_input(id!(message_input)).set_key_focus(cx);
        self.has_focus = true;
        self.redraw(cx);
    }

    pub fn text(&self) -> String {
        self.text_input(id!(message_input)).text()
    }
}

// 组件引用方法实现
impl InputBarRef {
    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn set_key_focus(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_key_focus(cx);
        }
    }

    pub fn text(&self) -> Option<String> {
        self.borrow().map(|inner| inner.text())
    }
}

use makepad_widgets::*;
use crate::profile::user_profile::{UserProfile};
use crate::profile::user_profile_cache::{get_user_profile_and_room_member};
use matrix_sdk::room::RoomMember;
use crate::shared::avatar::AvatarWidgetRefExt;
use matrix_sdk::ruma::{OwnedRoomId, RoomId};
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::avatar_cache::*;
use crate::utils;
use crate::shared::adaptive_view::DisplayContext;
use crate::shared::command_input_bar::*;

// 定义组件的视觉设计
live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::helpers::FillerX;
    use crate::shared::command_input_bar::*;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

    // // 定义用户列表项的视觉模板
    // UserListItem = <View> {
    //     width: Fill,
    //     height: Fit,
    //     padding: {left: 8., right: 8., top: 4., bottom: 4.}
    //     show_bg: true
    //     draw_bg: {color: #fff}
    //     flow: Down
    //     spacing: 8.0

    //     // 用户信息容器 (头像和用户名)
    //     user_info = <View> {
    //         width: Fill,
    //         height: Fit,
    //         flow: Right,
    //         spacing: 8.0
    //         align: {y: 0.5}

    //         avatar = <Avatar> {
    //             width: 24,
    //             height: 24,
    //             text_view = { text = { draw_text: {
    //                 text_style: { font_size: 12.0 }
    //             }}}
    //         }

    //         label = <Label> {
    //             height: Fit,
    //             draw_text: {
    //                 color: #000,
    //                 text_style: {font_size: 14.0}
    //             }
    //         }

    //         filler = <FillerX> {}
    //     }

    //     // Matrix ID 显示
    //     matrix_url = <Label> {
    //         height: Fit,
    //         draw_text: {
    //             color: #666,
    //             text_style: {font_size: 12.0}
    //         }
    //     }
    // }

    // 主组件设计
    pub MentionInputBar = {{MentionInputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        align: {y: 0.5}
        padding: 10.
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        // 位置按钮
        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        // user_list_item: <UserListItem> {}

        // 消息输入区域
        message_input = <CommandInputBar> {
            width: Fill,
            height: Fit
            margin: 0
            align: {y: 0.5}
            trigger: "@"
            keyboard_focus_color: (THEME_COLOR_CTRL_HOVER)
            pointer_hover_color: (THEME_COLOR_CTRL_HOVER * 0.85)

            popup = {
                show_bg: true
                draw_bg: { color: #fff }
                padding: { top: 4.0, bottom: 4.0 }

                list = {
                    width: Fill
                    height: 200.
                }
            }

            persistent = {
                center = {
                    text_input = {
                        empty_message: "Write a message (in Markdown) ..."
                        draw_bg: {color: (COLOR_PRIMARY)}
                        draw_text: {
                            color: (#000)
                            text_style: <MESSAGE_TEXT_STYLE>{}
                        }
                    }
                }
            }
        }

        // 发送按钮
        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

// 组件动作定义
#[derive(Clone, Debug)]
pub enum MentionInputBarAction {
    MessageChanged(String),
    UserMentioned(String),
}

// 组件主结构
#[derive(Live, Widget)]
pub struct MentionInputBar {
    #[deref]
    view: View,
    // 用户列表项模板
    #[live]
    user_list_item: Option<LivePtr>,
    // 组件状态
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    room_members: Vec<RoomMember>,
    #[rust]
    current_input: String,
    #[rust]
    mention_start_index: Option<usize>,
    #[rust]
    is_searching: bool,
}

// 实现组件的 LiveHook 特性
impl LiveHook for MentionInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        // 设置初始焦点到输入框
        self.command_input_bar(id!(message_input))
            .text_input_ref()
            .set_key_focus(cx);
    }
}

// 实现组件的 Widget 特性
impl Widget for MentionInputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let mut ret = self.view.draw_walk(cx, scope, walk);

        // 处理输入法编辑器位置
        let message_input = self.command_input_bar(id!(message_input));
        let text_input = message_input.text_input_ref();
        let area = text_input.area();
        cx.show_text_ime(area, DVec2::default());

        // 完成所有子组件的绘制
        while !ret.is_done() {
            ret = self.view.draw_walk(cx, scope, walk);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let widget_uid = self.widget_uid();
        let command_input = self.command_input_bar(id!(message_input));

        if let Event::Actions(actions) = event {
            // 1. 检查是否需要构建项目列表
            if command_input.should_build_items(actions) {
                let search_text = command_input.search_text();
                log!("Should build items triggered with search text: {}", search_text);

                self.show_user_list(cx, command_input.clone(), &search_text);
                return;
            }

            // 2. 处理文本变化
            if let Some(action) = actions.find_widget_action(command_input.text_input_ref().widget_uid()) {
                if let TextInputAction::Change(text) = action.cast() {
                    self.current_input = text.clone();

                    // 检查是否应该隐藏弹出框
                    if !text.contains('@') {
                        command_input.reset(cx);
                    }

                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        MentionInputBarAction::MessageChanged(text),
                    );
                }
            }

            // 3. 处理用户选择
            if let Some(selected) = command_input.item_selected(actions) {
                let username = selected.label(id!(user_info.label)).text();

                // 在选择后重置输入状态
                command_input.reset(cx);

                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    MentionInputBarAction::UserMentioned(username),
                );
            }
        }

        // 4. 处理键盘事件
        if let Event::KeyDown(key_event) = event {
            match key_event.key_code {
                KeyCode::Escape => {
                    // 按ESC时重置状态
                    command_input.reset(cx);
                },
                _ => {}
            }
        }

        self.view.handle_event(cx, event, scope);
    }
}

// 组件方法实现
impl MentionInputBar {
    // 显示用户列表
    pub fn show_user_list(&mut self, cx: &mut Cx, mut command_input: CommandInputBarRef, search_text: &str) {
        log!("show_user_list called with search text: {}", search_text);
        log!("Current room members count: {}", self.room_members.len());

        command_input.clear_items(cx);

        // Add debug logging for popup visibility
        let popup_visible = command_input.view(id!(popup)).visible();
        log!("Popup visibility before update: {}", popup_visible);

        if !search_text.is_empty() || command_input.text().ends_with('@') {
            command_input.view(id!(popup)).set_visible(true);

            // Filter and create items
            let filtered_users = self.room_members.iter()
                .filter(|member| {
                    let display_name = member.display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| member.user_id().to_string());
                    display_name.to_lowercase().contains(&search_text.to_lowercase())
                });

            // Add filtered users to the list
            for member in filtered_users {
                if let Some(item) = self.create_user_list_item(cx, member,
                    &member.display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| member.user_id().to_string())) {
                    log!("Adding user item to list: {}", member.user_id());
                    command_input.add_item(cx, item);
                }
            }

            // Verify popup visibility after updates
            let popup_visible_after = command_input.view(id!(popup)).visible();
            log!("Popup visibility after update: {}", popup_visible_after);

            self.redraw(cx);
        } else {
            command_input.view(id!(popup)).set_visible(false);
            self.redraw(cx);
        }
    }

    // 更新用户列表内容
    fn update_user_list(&mut self, cx: &mut Cx, mut command_input: CommandInputBarRef, search_text: &str) {
        log!("update_user_list called with search text: {}", search_text);
        // Clear existing items
        command_input.clear_items(cx);

        // Filter and add matching users
        let filtered_users = self.room_members.iter()
            .filter_map(|member| {
                let display_name = member.display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| member.user_id().to_string());

                if display_name.to_lowercase().contains(&search_text.to_lowercase()) {
                    log!("Found matching user: {}", display_name);
                    Some((member, display_name))
                } else {
                    None
                }
            });

        // Add filtered users to the list
        for (member, display_name) in filtered_users {
            if let Some(item) = self.create_user_list_item(cx, member, &display_name) {
                log!("Adding item for user: {}", display_name);
                command_input.add_item(cx, item);
            }
        }
    }

    fn create_user_list_item(&self, cx: &mut Cx, member: &RoomMember, display_name: &str) -> Option<WidgetRef> {
        let item = WidgetRef::new_from_ptr(cx, self.user_list_item);

        // 设置基本信息
        item.label(id!(label)).set_text(display_name);
        item.label(id!(matrix_url)).set_text(&format!("{}:matrix.org", member.user_id()));

        // 设置外观样式
        let is_desktop = cx.has_global::<DisplayContext>() &&
            cx.get_global::<DisplayContext>().is_desktop();

        if is_desktop {
            item.apply_over(cx, live!(
                flow: Right,
                align: {y: 0.5}
            ));
            // item.view(id!(user_info.filler)).set_visible(true);
        } else {
            item.apply_over(cx, live!(
                flow: Down,
                spacing: 4.0
            ));
            // item.view(id!(user_info.filler)).set_visible(false);
            item.label(id!(matrix_url)).apply_over(cx, live!(
                margin: {left: 0.}
            ));
        }

        // 设置头像
        let avatar = item.avatar(id!(avatar));
        if let Some(mxc_uri) = member.avatar_url() {
            if let Some(avatar_data) = get_avatar(cx, mxc_uri) {
                let _ = avatar.show_image(None, |img| {
                    utils::load_png_or_jpg(&img, cx, &avatar_data)
                });
            } else {
                avatar.show_text(None, display_name);
            }
        } else {
            avatar.show_text(None, display_name);
        }

        Some(item)
    }

    // 公共接口方法
    pub fn text(&self) -> String {
        self.command_input_bar(id!(message_input))
            .text_input_ref()
            .text()
    }

    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.command_input_bar(id!(message_input))
            .text_input_ref()
            .set_text_and_redraw(cx, text);
    }

    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());
        log!("Setting room id {} ", room_id.clone() );
        submit_async_request(MatrixRequest::FetchRoomMembers {
            room_id: room_id
        });
    }

    pub fn set_room_members(&mut self, members: Vec<RoomMember>) {
        log!("Setting {} members to MentionInputBar", members.len());
        self.room_members = members;
    }
}

// 组件引用的方法实现
impl MentionInputBarRef {
    pub fn text(&self) -> Option<String> {
        self.borrow().map(|inner| inner.text())
    }

    pub fn set_text(&self, cx: &mut Cx, text: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_text(cx, text);
        }
    }

    pub fn set_room_id(&self, room_id: OwnedRoomId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_id(room_id);
        }
    }

    pub fn set_room_members(&self, members: Vec<RoomMember>) {
        log!("Setting room members ref ... ");
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(members);
        }
    }
}

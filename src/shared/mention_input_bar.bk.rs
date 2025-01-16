use makepad_widgets::*;
use matrix_sdk::room::RoomMember;
use crate::shared::avatar::AvatarWidgetRefExt;
use matrix_sdk::ruma::{OwnedRoomId, RoomId};
use crate::sliding_sync::{submit_async_request, MatrixRequest};
use crate::avatar_cache::*;
use crate::utils;
use crate::shared::adaptive_view::DisplayContext;


live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::shared::styles::*;
    use crate::shared::icon_button::*;
    use crate::shared::avatar::Avatar;
    use crate::shared::adaptive_view::*;

    ICO_LOCATION_PERSON = dep("crate://self/resources/icons/location-person.svg")
    ICO_SEND = dep("crate://self/resources/icon_send.svg")

    // // 用户列表项模板定义
    // UserListItem = <View> {
    //     width: Fill,
    //     height: Fit,
    //     padding: {left: 8., right: 8., top: 4., bottom: 4.}
    //     show_bg: true
    //     draw_bg: {color: #fff}
    //     flow: Right
    //     spacing: 8.0

    //     // 左侧头像
    //     avatar = <Avatar> {
    //         width: 24,
    //         height: 24,
    //         text_view = { text = { draw_text: {
    //             text_style: { font_size: 12.0 }
    //         }}}
    //     }

    //     // 中间使用一个容器包含用户名
    //     name_container = <View> {
    //         width: Fill  // 让这个容器填充剩余空间
    //         height: Fit
    //         flow: Down   // 使用垂直布局
    //         spacing: 2.0 // 用户名和Matrix URL之间的间距

    //         // 用户名标签
    //         label = <Label> {
    //             width: Fill,
    //             height: Fit,
    //             draw_text: {
    //                 color: #000,
    //                 text_style: {font_size: 14.0}
    //             }
    //         }

    //         // Matrix URL标签
    //         matrix_url = <Label> {
    //             width: Fill,
    //             height: Fit,
    //             draw_text: {
    //                 color: #666,  // 使用灰色显示Matrix URL
    //                 text_style: {font_size: 12.0}  // 使用较小的斜体字
    //             }
    //         }
    //     }
    // }


    // 视觉容器：专门处理背景和视觉相关的属性
    BaseListItem = <View> {
        width: Fill,
        height: Fit,
        padding: {left: 8., right: 8., top: 4., bottom: 4.}
        show_bg: true
        draw_bg: {color: #fff}
    }


    pub MentionInputBar = {{MentionInputBar}} {
        width: Fill,
        height: Fit
        flow: Right
        align: {y: 0.5}
        padding: 10.
        show_bg: true
        draw_bg: {color: (COLOR_PRIMARY)}

        // 位置按钮配置
        location_button = <IconButton> {
            draw_icon: {svg_file: (ICO_LOCATION_PERSON)},
            icon_walk: {width: 22.0, height: Fit, margin: {left: 0, right: 5}},
            text: "",
        }

        // user_list_item: <UserListItem> {}

        user_list_item: <AdaptiveView> {
            Desktop = <BaseListItem> {
                flow: Right
                spacing: 8.0

                left_container = <View> {
                    flow: Right
                    spacing: 8.0

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
                }

                matrix_url = <Label> {
                    width: Fit,
                    height: Fit,
                    draw_text: {
                        color: #666,
                        text_style: {font_size: 12.0}
                    }
                }

            }

            Mobile = <BaseListItem> {
                flow: Down
                spacing: 2.0

                top_container = <View> {
                    flow: Right
                    spacing: 8.0

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
                }

                matrix_url = <Label> {
                    width: Fill,
                    height: Fit,
                    margin: {left: 32}
                    draw_text: {
                        color: #666,
                        text_style: {font_size: 12.0}
                    }
                }
            }
        }

        message_input = <CommandTextInput> {
            width: Fill,
            height: Fit
            margin: 0
            align: {y: 0.5}
            trigger: "@"
            keyboard_focus_color: (THEME_COLOR_CTRL_HOVER)
            pointer_hover_color: (THEME_COLOR_CTRL_HOVER * 0.85)

            // Configure the popup search area
            popup = {
                search_input = {
                    empty_message: "Search users..."
                    draw_bg: {color: #fff}
                }

                list = {
                    height: 200.0  // Fixed height in pixels
                    clip_y: true
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

        // 发送按钮配置
        send_message_button = <IconButton> {
            draw_icon: {svg_file: (ICO_SEND)},
            icon_walk: {width: 18.0, height: Fit},
        }
    }
}

// Define the actions that our component can emit
#[derive(Clone, Debug)]
pub enum MentionInputBarAction {
    MessageChanged(String),
    UserMentioned(String),
}

// Main component implementation
#[derive(Live, Widget)]
pub struct MentionInputBar {
    #[deref]
    view: View,
    // Store the template for user list items
    #[live]
    user_list_item: Option<LivePtr>,
    #[rust]
    room_id: Option<OwnedRoomId>,
    #[rust]
    room_members: Vec<RoomMember>,
    #[rust]
    current_input: String,
    #[rust]
    mention_start_index: Option<usize>,
}

impl LiveHook for MentionInputBar {
    fn after_new_from_doc(&mut self, cx: &mut Cx) {
        // Set initial focus to the input field
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .set_key_focus(cx);
    }
}

impl Widget for MentionInputBar {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // 首先开始绘制基础视图
        let mut ret = self.view.draw_walk(cx, scope, walk);

        // 获取文本输入组件的引用并处理 IME
        let message_input = self.command_text_input(id!(message_input));
        let text_input = message_input.text_input_ref();

        // 获取输入区域并设置 IME 位置
        let area = text_input.area();
        cx.show_text_ime(area, DVec2::default());

        // 继续绘制，直到所有子组件都完成绘制
        while !ret.is_done() {
            ret = self.view.draw_walk(cx, scope, walk);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Get reference to our input component
        let widget_uid = self.widget_uid();
        let mut message_input = self.command_text_input(id!(message_input));

        if let Event::Actions(actions) = event {
            // Handle user selection from popup list
            if let Some(selected) = message_input.item_selected(actions) {
                let username = selected.label(id!(label)).text();

                // Insert the mention at the current cursor position
                if let Some(start_idx) = self.mention_start_index {
                    let current_text = self.current_input.clone();
                    let before_mention = &current_text[..start_idx];
                    let after_mention = &current_text[message_input.text_input_ref().borrow()
                        .map_or(0, |p| p.get_cursor().head.index)..];

                    // Format the new text with the mention
                    let new_text = format!("{}@{}{}", before_mention, username, after_mention);
                    self.set_text(cx, &new_text);

                    // Reset mention state
                    self.mention_start_index = None;
                }

                cx.widget_action(
                    widget_uid,
                    &scope.path,
                    MentionInputBarAction::UserMentioned(username),
                );
            }


            // Handle updating the user list when searching
            if message_input.should_build_items(actions) {
                message_input.clear_items();

                // Get the search text for filtering
                let search_text = message_input.search_text().to_lowercase();

                log!("Building user list for search text: '{}', have {} members",
                        search_text, self.room_members.len());
                // Example user list - in a real app, this would come from your data source
                // let users = vec!["Alice", "Bob", "Charlie", "David", "Eve"];

                // // Filter and add matching users to the popup list
                // for username in users {
                //     if username.to_lowercase().contains(&search_text) {
                //         // Create a new list item from our template
                //         let item = WidgetRef::new_from_ptr(cx, self.user_list_item);
                //         item.label(id!(label)).set_text(username);
                //         message_input.add_item(item);
                //     }
                // }

                // Filter room members based on search text
                for member in &self.room_members {
                    let display_name = member.display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| member.user_id().to_string());

                    if display_name.to_lowercase().contains(&search_text) {
                        // Create list item with avatar and name
                        let item = WidgetRef::new_from_ptr(cx, self.user_list_item);
                        // 设置Matrix URL
                        let matrix_url = format!("{}:matrix.org", member.user_id());


                        // 根据布局设置不同的路径
                        if cx.get_global::<DisplayContext>().is_desktop() {
                            item.label(id!(left_container.label)).set_text(&display_name);
                            item.label(id!(matrix_url)).set_text(&matrix_url);
                        } else {
                            item.label(id!(top_container.label)).set_text(&display_name);
                            item.label(id!(matrix_url)).set_text(&matrix_url);
                        }

                        // 设置头像的代码在两种模式下是一样的
                        let avatar = if cx.get_global::<DisplayContext>().is_desktop() {
                            item.avatar(id!(left_container.avatar))
                        } else {
                            item.avatar(id!(top_container.avatar))
                        };

                        let room_id : &RoomId = self.room_id.as_ref().unwrap();

                        log!("======= ROOM ID ======= : {:?}", room_id);

                        // 从 member 获取 mxc_uri
                        if let Some(mxc_uri) = member.avatar_url() {
                            // 从缓存中获取头像数据
                            if let Some(avatar_data) = get_avatar(cx, mxc_uri) {
                                // 如果缓存中有头像数据,显示图片
                                let _ = avatar.show_image(None, |img| {
                                    utils::load_png_or_jpg(&img, cx, &avatar_data)
                                });
                            } else {
                                // 如果缓存中没有,显示文本头像
                                avatar.show_text(None, &display_name);
                            }
                        } else {
                            // 如果没有设置头像,显示文本头像
                            avatar.show_text(None, &display_name);
                        }


                        message_input.add_item(item);
                    }
                }
            }

            // Handle text input changes
            if let Some(action) = actions.find_widget_action(message_input.text_input_ref().widget_uid()) {
                if let TextInputAction::Change(text) = action.cast() {
                    self.current_input = text.clone();

                    // Track mention start position when @ is typed
                    if text.ends_with('@') {
                        self.mention_start_index = Some(text.len() - 1);
                    }

                    cx.widget_action(
                        widget_uid,
                        &scope.path,
                        MentionInputBarAction::MessageChanged(text),
                    );
                }
            }
        }

        self.view.handle_event(cx, event, scope);
    }
}

// Implement public methods for the component
impl MentionInputBar {
    pub fn text(&self) -> String {
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .text()
    }

    pub fn set_text(&mut self, cx: &mut Cx, text: &str) {
        self.command_text_input(id!(message_input))
            .text_input_ref()
            .set_text_and_redraw(cx, text);
    }

    pub fn set_room_id(&mut self, room_id: OwnedRoomId) {
        self.room_id = Some(room_id.clone());

        submit_async_request(MatrixRequest::FetchRoomMembers {
            room_id: room_id
        });
    }

    pub fn set_room_members(&mut self, members: Vec<RoomMember>) {
        log!("Setting {} members to MentionInputBar", members.len());
        self.room_members = members;
    }
}

// Implement methods for component references
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
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_room_members(members);
        }
    }
}

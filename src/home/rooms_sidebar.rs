use makepad_widgets::*;
use crate::{home::rooms_list::{RoomsListWidgetExt, RoomsListWidgetRefExt}, shared::collapse_view::collapse::GCollapseWidgetExt};

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import makepad_draw::shader::std::*;

    import crate::shared::styles::*;
    import crate::shared::helpers::*;
    import crate::shared::adaptive_view::AdaptiveView;

    import crate::home::rooms_list::RoomsList;
    import crate::shared::cached_widget::CachedWidget;
    import crate::shared::collapse_view::GCollapse;

    ICON_COLLAPSE = dep("crate://self/resources/icons/collapse.svg")
    ICON_ADD = dep("crate://self/resources/icons/add.svg")

    CollapsableTitle = <View> {
        width: Fill, height: Fit
        flow: Right, spacing: 10.
        align: {x: 0.0, y: 0.5}
        collapse_icon = <Icon> {
            draw_icon: {
                svg_file: (ICON_COLLAPSE),
                uniform rotation_angle: -90.,
                fn get_color(self) -> vec4 {
                    // return #666;
                    return (COLOR_TEXT_IDLE);
                }

                // Support rotation of the icon
                fn clip_and_transform_vertex(self, rect_pos: vec2, rect_size: vec2) -> vec4 {
                    let clipped: vec2 = clamp(
                        self.geom_pos * rect_size + rect_pos,
                        self.draw_clip.xy,
                        self.draw_clip.zw
                    )
                    self.pos = (clipped - rect_pos) / rect_size

                    // Calculate the texture coordinates based on the rotation angle
                    let angle_rad = self.rotation_angle * 3.14159265359 / 180.0;
                    let cos_angle = cos(angle_rad);
                    let sin_angle = sin(angle_rad);
                    let rot_matrix = mat2(
                        cos_angle, -sin_angle,
                        sin_angle, cos_angle
                    );
                    self.tex_coord1 = mix(
                        self.icon_t1.xy,
                        self.icon_t2.xy,
                        (rot_matrix * (self.pos.xy - vec2(0.5))) + vec2(0.5)
                    );

                    return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                        clipped.x,
                        clipped.y,
                        self.draw_depth + self.draw_zbias,
                        1.
                    )))
                }
            }
            icon_walk: {width: 12, height: 12}
        }

        title = <Label> {
            draw_text: {
                color: #x0,
                text_style: <TITLE_TEXT>{ font_size: 11}
            }
        }

        <View> {
            width: Fill
        }

        add_icon = <View> {
            width: Fit
            visible: false
            padding: {right: 10}
            align: {x: 0.5, y: 0.5}
            <Icon> {
                icon_walk: {width: 10, height: 10}
                draw_icon: {
                    svg_file: (ICON_ADD),
                    fn get_color(self) -> vec4 {
                        return (COLOR_TEXT_IDLE);
                    }
                }
            }
        }
    }

    RoomsView = {{RoomsView}} {
        show_bg: true,
        draw_bg: {
            instance bg_color: (COLOR_PRIMARY)
            instance border_color: #f2f2f2
            instance border_width: 0.003

            // Draws a right-side border
            fn pixel(self) -> vec4 {
                if self.pos.x > 1.0 - self.border_width {
                    return self.border_color;
                } else {
                    return self.bg_color;
                }
            }
        }
        <Label> {
            text: "Home"
            draw_text: {
                color: #x0
                text_style: <TITLE_TEXT>{}
            }
        }
        <View> {
            flow: Down, spacing: 20
            padding: {top: 20}
            width: Fill, height: Fit

            people_collapse = <GCollapse> {
                opened: false

                header: {
                    <CollapsableTitle> {
                        title = {
                            text: "People"
                            draw_text: {
                                color: (COLOR_TEXT_IDLE)
                            }
                        }

                        collapse_icon = {
                            draw_icon: { rotation_angle: -90. }
                        }
                    }
                }

            }

            channels_collapse = <GCollapse> {
                opened: false

                header: {
                    <CollapsableTitle> {
                        title = {
                            text: "Channels"
                            draw_text: {
                                color: (COLOR_TEXT_IDLE)
                            }
                        }

                        collapse_icon = {
                            draw_icon: { rotation_angle: -90. }
                        }
                    }
                }

            }

            rooms_collapse = <GCollapse> {
                height: Fit,
                width: Fill,
                opened: true

                header: {
                    <CollapsableTitle> {
                        title = {
                            text: "Rooms (3 rooms)"
                            draw_text: {
                                color: #666666
                            }
                        }
                        collapse_icon = {
                            draw_icon: { rotation_angle: 0. }
                        }
                        add_icon = {
                            visible: true
                        }


                    }
                }
                body : {
                    // Fixed ME: `<CachedWidget>` dont work
                    <View> {
                        // FIXED ME: `height: Fit` dont work
                        height: 350.,
                        width: Fill,
                        // show_bg: true,
                        // draw_bg: {
                        //     color: #ff0000
                        // }
                        rooms_list = <RoomsList> {}
                    }
                }
            }
        }

    }

    RoomsSideBar = <AdaptiveView> {
        Desktop = <RoomsView> {
            padding: {top: 20., left: 10., right: 10.}
            flow: Down, spacing: 10
            width: Fill, height: Fill
        },
        Mobile = <RoomsView> {
            padding: {top: 17., left: 17., right: 17.}
            flow: Down, spacing: 7
            width: Fill, height: Fill
        }
    }
}

#[derive(Widget, Live, LiveHook)]
pub struct RoomsView {
    #[deref]
    view: View,
    #[rust(false)]
    rooms_collapsed: bool,
    #[rust]
    room_count: usize,
}

impl Widget for RoomsView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let collapse = self.view.gcollapse(id!(rooms_collapse));
        let header = collapse.view(id!(header));
        let icon = header.view(id!(collapse_icon));
        self.room_count = 0;

        if let Some(collapse_inner) = collapse.borrow() {
            self.rooms_collapsed = !collapse_inner.opened;
            // Update room count
            // Access the rooms_list from the collapse body
            let rooms_list = collapse_inner.body.view(id!(rooms_list)).as_rooms_list();
            if let Some(rooms_list) = rooms_list.borrow() {
                self.room_count = rooms_list.rooms_count();
            };

        }
        log!("apply =======> rooms_collapsed {:?}", self.rooms_collapsed);

        match event.hits(cx, icon.area()) {
            Hit::FingerDown(_) => {

                if self.rooms_collapsed {
                    collapse.animate_open_off(cx);
                } else {
                    collapse.animate_open_on(cx);
                }

                self.rooms_collapsed = !self.rooms_collapsed;

                if let Some(mut collapse_inner_mut) = collapse.borrow_mut() {
                    collapse_inner_mut.opened = !self.rooms_collapsed;
                }

                log!("apply =======> FingerDown rooms_collapsed {:?}", self.rooms_collapsed);
                return
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerHoverOut(_) => {
                cx.set_cursor(MouseCursor::Default);
            }
            _ => {}
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let collapse = self.view.gcollapse(id!(rooms_collapse));
        let rotation = if self.rooms_collapsed {
            -90.0
        } else {
            0.0
        };

        // let rooms_list = self.view(id!(rooms_list));
        // let rect = rooms_list. get_rect(cx);
        // log!("RoomsList height: {}", rect.size.y);

        if let Some(collapse_inner) = collapse.borrow() {
            // let icon = collapse.view(id!(header)).view(id!(collapse_icon));
            log!("apply =======> rotation {:?}", self.rooms_collapsed);

            collapse_inner.header.icon(id!(collapse_icon)).apply_over(cx, live! {
                draw_icon: { rotation_angle: (rotation) }
            });

            // Update the title text before drawing
            let title_text = format!("Rooms ({} rooms)", self.room_count);
            collapse_inner.header.label(id!(title)).apply_over(cx, live! {
                text: (title_text)
            });
        }






        log!("apply =======> rotation {:?}", self.rooms_collapsed);
        self.view.draw_walk(cx, scope, walk)
    }
}

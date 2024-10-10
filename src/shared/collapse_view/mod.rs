use makepad_widgets::*;

pub mod event;
pub mod register;
pub mod types;
pub mod utils;
pub mod collapse;

pub use register::register;

live_design! {
    import makepad_widgets::base::*;
    import makepad_widgets::theme_desktop_dark::*;
    import crate::shared::collapse_view::collapse::GCollapseBase;

    ALIGN_LEFT_WALK = {x: 0.0, y: 0.5};

    GCollapse = <GCollapseBase>{

        height: Fit,
        width: Fill,
        flow: Down,
        opened: true,
        header:  <View>{
            height: 24.0,
            padding: {left: 6.0, right: 6.0, top: 3.0, bottom: 3.0},
            flow: Right,
            align: <ALIGN_LEFT_WALK>{},
            spacing: 6.0,
            margin: 0.0,

        },
        body: <View>{
            height: 800.0,
            width: Fill,
            padding: {left: 6.0, right: 6.0, top: 3.0, bottom: 3.0},
            margin: 0.0,

        }
    }

}

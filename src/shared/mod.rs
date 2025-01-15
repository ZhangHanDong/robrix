use makepad_widgets::Cx;

pub mod adaptive_view;
pub mod avatar;
pub mod clickable_view;
pub mod color_tooltip;
pub mod helpers;
pub mod html_or_plaintext;
pub mod icon_button;
pub mod jump_to_bottom_button;
pub mod search_bar;
pub mod styles;
pub mod text_or_image;
pub mod typing_animation;
pub mod verification_badge;
pub mod input_bar;
pub mod mention_input_bar;

pub fn live_design(cx: &mut Cx) {
    // Order matters here, as some widget definitions depend on others.
    styles::live_design(cx);
    helpers::live_design(cx);
    icon_button::live_design(cx);
    search_bar::live_design(cx);
    clickable_view::live_design(cx);
    avatar::live_design(cx);
    text_or_image::live_design(cx);
    html_or_plaintext::live_design(cx);
    adaptive_view::live_design(cx);
    typing_animation::live_design(cx);
    jump_to_bottom_button::live_design(cx);
    verification_badge::live_design(cx);
    color_tooltip::live_design(cx);
    input_bar::live_design(cx);
    mention_input_bar::live_design(cx);
}

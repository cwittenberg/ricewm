use super::*;
use rice_ui::{Point, Rect, Size};
fn field() -> Rect {Rect {origin:Point{x:0,y:0},size:Size{width:101,height:101}}}
#[test]
fn color_plane_maps_saturation_and_value_and_preserves_hue_at_white() {
    assert_eq!(continuous_color_value("color-plane:#00ff00@120",field(),Point{x:50,y:50}),"#408040@120");
    let white=continuous_color_value("color-plane:#00ff00@120",field(),Point{x:0,y:0});
    assert_eq!(white,"#ffffff@120");
    assert_eq!(continuous_color_value(&format!("color-plane:{white}"),field(),Point{x:100,y:0}),"#00ff00@120");
    assert_eq!(continuous_color_value("color-plane:#00ff00@120",field(),Point{x:200,y:200}),"#000000@120");
}
#[test]
fn hue_strip_selects_a_visible_color_from_grayscale() {
    assert_eq!(continuous_color_value("color-hue:#ffffff@0",field(),Point{x:50,y:10}),"#00ffff@180");
}

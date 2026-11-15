//! Bounded native chip presentation shared by actions and passive badges.
use rice_ui::{Color, Point, Rect};

pub(super) struct Chip<'a> {
    pub radius: u16,
    pub fill: Option<Color>,
    pub foreground: Option<Color>,
    pub removable: bool,
    pub label: &'a str,
}
pub(super) fn chip(text: Option<&str>) -> Option<Chip<'_>> {
    let mut parts = text?.strip_prefix("visual-chip:")?.splitn(5, ':');
    let radius = parts.next()?.parse::<u16>().ok()?;
    let color = |value: &str| -> Option<Option<Color>> {
        if value == "-" { return Some(None); }
        if value.len() != 6 || !value.is_ascii() { return None; }
        let n = u32::from_str_radix(value,16).ok()?;
        Some(Some(Color::opaque((n>>16) as u8,(n>>8) as u8,n as u8)))
    };
    Some(Chip {radius,fill:color(parts.next()?)?,foreground:color(parts.next()?)?,removable:match parts.next()? {"0"=>false,"1"=>true,_=>return None},label:parts.next()?})
}
pub(super) fn contains(rect: Rect, point: Point, radius: u16) -> bool {
    if !rect.contains(point) { return false; }
    let r = f64::from(u32::from(radius.min(32)).min(rect.size.width/2).min(rect.size.height/2));
    let x=f64::from(point.x)-f64::from(rect.origin.x);
    let y=f64::from(point.y)-f64::from(rect.origin.y);
    let dx=(r-x).max(x-(f64::from(rect.size.width)-r)).max(0.0);
    let dy=(r-y).max(y-(f64::from(rect.size.height)-r)).max(0.0);
    dx*dx+dy*dy<=r*r
}

pub(super) fn contrast_ink(color: Color) -> Color {
    let [r,g,b,_]=color.channels();
    let linear=|channel:u8| {let value=f64::from(channel)/255.0;
        if value<=0.04045 {value/12.92} else {((value+0.055)/1.055).powf(2.4)}};
    if linear(r)*0.2126+linear(g)*0.7152+linear(b)*0.0722>0.179 {
        Color::opaque(0,0,0)
    } else {Color::opaque(255,255,255)}
}

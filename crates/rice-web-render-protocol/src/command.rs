/// A validated retained-rendering command borrowed from a display-list
/// transaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command<'a> {
    /// Live browser document; never replayed as canvas paint.
    HtmlDocument { html: &'a str },
    PushStackingContext {
        transform: [f32; 6],
        opacity: f32,
        /// Peniko/W3C mixing and Porter-Duff composition discriminants.
        blend: [u8; 2],
    },
    PopStackingContext,
    /// Outer shadow of the alpha composited by the enclosed paint commands.
    PushDropShadow { bounds: [f32; 4], offset: [f32; 2], blur: f32, color: u32, transform: Option<[f32; 6]> },
    PopDropShadow,
    PushClip {
        rect: [f32; 4],
        radii: [f32; 4],
    },
    PopClip,
    SolidRect {
        rect: [f32; 4],
        color: u32,
    },
    LinearGradient {
        rect: [f32; 4],
        start: [f32; 2],
        end: [f32; 2],
        stops: &'a [u8],
    },
    RadialGradient {
        rect: [f32; 4],
        start_center: [f32; 2],
        start_radius: f32,
        end_center: [f32; 2],
        end_radius: f32,
        stops: &'a [u8],
    },
    SweepGradient {
        rect: [f32; 4],
        center: [f32; 2],
        start_angle: f32,
        end_angle: f32,
        extend: u8,
        stops: &'a [u8],
    },
    Image {
        rect: [f32; 4],
        uv: [f32; 4],
        texture: u32,
        alpha: f32,
        sampling: u8,
    },
    DefineImage {
        texture: u32,
        width: u32,
        height: u32,
        rgba: &'a [u8],
    },
    HitRegion {
        rect: [f32; 4],
        node: u64,
        kind: u8,
    },
    LinkRegion {
        rect: [f32; 4],
        node: u64,
        url: &'a [u8],
    },
    TitleRegion {
        rect: [f32; 4],
        node: u64,
        title: &'a [u8],
    },
    MediaSurface {
        rect: [f32; 4],
        session: u64,
    },
    ImageRegion {
        rect: [f32; 4],
        node: u64,
        url: &'a [u8],
    },
    InputRegion {
        rect: [f32; 4],
        node: u64,
        kind: u8,
        flags: u8,
        value: &'a [u8],
    },
    DocumentMetrics {
        content_width: u32,
        content_height: u32,
        scroll_x: u32,
        scroll_y: u32,
    },
    GlyphRun {
        font_instance: u32,
        origin: [f32; 2],
        color: u32,
        glyphs: &'a [u8],
    },
    Border {
        rect: [f32; 4],
        widths: [f32; 4],
        colors: [u32; 4],
        radii: [f32; 4],
    },
    BoxShadow {
        rect: [f32; 4],
        offset: [f32; 2],
        blur: f32,
        spread: f32,
        color: u32,
        inset: bool,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: u32,
        style: u8,
    },
    Polygon {
        color: u32,
        stroke_width: f32,
        filled: bool,
        closed: bool,
        points: &'a [u8],
    },
    Arc {
        rect: [f32; 4],
        start_degrees: f32,
        end_degrees: f32,
        width: f32,
        color: u32,
        filled: bool,
    },
    DefinePath {
        path: u32,
        segments: &'a [u8],
    },
    PushPathClip {
        path: u32,
        transform: [f32; 6],
        fill_rule: u8,
    },
    FillPath {
        path: u32,
        transform: [f32; 6],
        color: u32,
        fill_rule: u8,
    },
    StrokePath {
        path: u32,
        transform: [f32; 6],
        color: u32,
        width: f32,
        miter_limit: f32,
        line_cap: u8,
        line_join: u8,
    },
}

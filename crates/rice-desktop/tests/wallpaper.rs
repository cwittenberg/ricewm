use rice_desktop::{
    Color, Rect, Size, Wallpaper, WallpaperLayout, WallpaperMode, WallpaperSettings, WorkspaceId,
    wallpaper::{DefaultWallpaperLayout, WallpaperLayoutStrategy},
};

fn image(mode: WallpaperMode) -> Wallpaper {
    Wallpaper::Image {
        resource: String::from("wallpaper"),
        mode,
        background: Color::rgb(1, 2, 3),
    }
}

#[test]
fn all_image_modes_have_deterministic_geometry() {
    let strategy = DefaultWallpaperLayout;
    let viewport = Size::new(1600, 900);
    let intrinsic = Some(Size::new(800, 600));

    let WallpaperLayout::Image { placement, .. } =
        strategy.layout(&image(WallpaperMode::Center), viewport, intrinsic)
    else {
        panic!("center layout");
    };
    assert_eq!(placement.destination, Rect::new(400, 150, 800, 600));

    let WallpaperLayout::Image { placement, .. } =
        strategy.layout(&image(WallpaperMode::Stretch), viewport, intrinsic)
    else {
        panic!("stretch layout");
    };
    assert_eq!(placement.destination, Rect::new(0, 0, 1600, 900));
    assert_eq!(placement.source, Rect::new(0, 0, 800, 600));

    let WallpaperLayout::Image { placement, .. } =
        strategy.layout(&image(WallpaperMode::Fit), viewport, intrinsic)
    else {
        panic!("fit layout");
    };
    assert_eq!(placement.destination, Rect::new(200, 0, 1200, 900));

    let WallpaperLayout::Image { placement, .. } =
        strategy.layout(&image(WallpaperMode::Crop), viewport, intrinsic)
    else {
        panic!("crop layout");
    };
    assert_eq!(placement.source, Rect::new(0, 75, 800, 450));
    assert_eq!(placement.destination, Rect::new(0, 0, 1600, 900));

    let WallpaperLayout::Tiled {
        tile_size, clip, ..
    } = strategy.layout(&image(WallpaperMode::Tile), viewport, intrinsic)
    else {
        panic!("tile layout");
    };
    assert_eq!(tile_size, Size::new(800, 600));
    assert_eq!(clip, Rect::new(0, 0, 1600, 900));
}

#[test]
fn centered_oversize_image_uses_signed_origin() {
    let WallpaperLayout::Image { placement, .. } = DefaultWallpaperLayout.layout(
        &image(WallpaperMode::Center),
        Size::new(640, 480),
        Some(Size::new(1000, 800)),
    ) else {
        panic!("image layout");
    };
    assert_eq!(placement.destination, Rect::new(-180, -160, 1000, 800));
}

#[test]
fn missing_image_size_falls_back_to_background() {
    let WallpaperLayout::Solid { color, viewport } =
        DefaultWallpaperLayout.layout(&image(WallpaperMode::Fit), Size::new(320, 200), None)
    else {
        panic!("solid fallback");
    };
    assert_eq!(color, Color::rgb(1, 2, 3));
    assert_eq!(viewport, Rect::new(0, 0, 320, 200));
}

#[test]
fn workspace_override_is_sparse_and_reversible() {
    let common = Wallpaper::Solid(Color::rgb(10, 20, 30));
    let alternate = image(WallpaperMode::Crop);
    let one = WorkspaceId::new("1");
    let two = WorkspaceId::new("2");
    let mut settings = WallpaperSettings::new(common.clone());

    assert_eq!(settings.for_workspace(&one), &common);
    settings.set_workspace(one.clone(), alternate.clone());
    assert_eq!(settings.for_workspace(&one), &alternate);
    assert_eq!(settings.for_workspace(&two), &common);
    assert_eq!(settings.clear_workspace(&one), Some(alternate));
    assert_eq!(settings.for_workspace(&one), &common);
}

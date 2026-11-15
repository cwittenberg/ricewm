use super::*;
#[test]
fn taskbar_geometry_tracks_real_edges_dock_and_hidden_state() {
    let mut server=DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
    for (wire,edge) in [(0,3),(1,1),(2,0),(3,2)] {
        assert!(server.set_taskbar_configuration(true,wire,false));
        let geometry=server.taskbar_geometry().unwrap();let window=&server.windows[&WM_TASKBAR.0];
        assert_eq!(geometry,[edge,i32::from(window.x),i32::from(window.y),i32::from(window.x)+i32::from(window.width),i32::from(window.y)+i32::from(window.height),1,0,1]);
    }
    server.wm_dock_style=true;server.refresh_taskbar_geometry();
    let dock=server.taskbar_geometry().unwrap();assert!(dock[4]-dock[2]<600);
    assert!(server.set_taskbar_configuration(true,0,true));
    let hidden=server.taskbar_geometry().unwrap();assert_eq!(&hidden[5..],&[1,1,0]);assert!(hidden[3]>hidden[1]&&hidden[4]>hidden[2]);
    server.wm_taskbar_revealed=true;server.refresh_taskbar_geometry();
    let visible=server.taskbar_geometry().unwrap();assert_eq!(&visible[..7],&hidden[..7]);assert_eq!(visible[7],1);
    server.windows.remove(&WM_TASKBAR.0);assert!(server.taskbar_geometry().is_none());
}

#[test]
fn compact_shell_preserves_tasks_and_bounds_readable_cascades() {
    for width in [320, 390, 600, 768, 1024, 1440] {
        let mut server = DisplayServer::new(DisplayConfig {width_px:width,height_px:844,native_window_manager:true,..DisplayConfig::default()});
        if width < 700 {
            assert!(server.wm_tray_layout().is_empty());
            assert!(server.wm_vertical_tray_layout().is_empty());
            assert!(!server.wm_root_rows().contains(&WmMenuRow::WorkspaceNavigation));
            server.wm_menu_level = WmMenuLevel::Applications(1);
            let widths = server.wm_menu_column_widths();
            let offsets = server.wm_menu_column_x_offsets();
            for (&size, &left) in widths.iter().zip(&offsets) {
                assert!(size >= 180, "cascade collapsed at {width}: {widths:?}");
                assert!(left + size <= usize::from(width));
            }
        } else {
            assert!(!server.wm_tray_layout().is_empty());
            assert!(server.wm_root_rows().contains(&WmMenuRow::WorkspaceNavigation));
        }
    }
}

#[test]
fn menu_text_carries_exact_column_layers_at_every_size() {
    for width in [320, 448, 1440] {
        let mut server = DisplayServer::new(DisplayConfig {width_px:width,height_px:844,native_window_manager:true,..DisplayConfig::default()});
        server.wm_menu_level = WmMenuLevel::Themes;
        server.relayout_wm_menu();
        let layers = server.wm_text_overlays.iter().filter(|row| row.window == WM_MENU && row.text == b"__menu_layer").collect::<Vec<_>>();
        {
            let widths = server.wm_menu_column_widths();
            let xs = server.wm_menu_column_x_offsets();
            let ys = server.wm_menu_column_offsets();
            assert_eq!(layers.len(), widths.len());
            for (index, layer) in layers.iter().enumerate() {
                assert_eq!(usize::from(layer.x), xs[index]);
                assert_eq!(usize::from(layer.y), ys[index]);
                assert_eq!(usize::from(layer.width), widths[index]);
                assert!(layer.height > 0);
            }
        }
    }
}

#[test]
fn compact_third_menu_reveals_parent_without_moving_it() {
    for (width, height) in [(240,568), (320,568), (448,586), (600,844), (844,390)] {
        let mut server = DisplayServer::new(DisplayConfig {width_px:width,height_px:height,native_window_manager:true,..DisplayConfig::default()});
        server.wm_menu_level = WmMenuLevel::Categories;
        let parent_widths = server.wm_menu_column_widths();
        let parent_offsets = server.wm_menu_column_x_offsets();
        server.wm_menu_level = WmMenuLevel::Applications(1);
        let widths = server.wm_menu_column_widths();
        let offsets = server.wm_menu_column_x_offsets();
        assert_eq!(&widths[..2], parent_widths.as_slice(), "parent widths changed at {width}");
        assert_eq!(&offsets[..2], parent_offsets.as_slice(), "parent positions changed at {width}");
        assert!(offsets[2].abs_diff(offsets[1]) >= 32.min(usize::from(width - 8) / 8), "third pane hides its parent at {width}: {offsets:?}");
        for (x, w) in offsets.iter().zip(widths) {
            assert!(x + w <= usize::from(width - 8));
        }
    }
}

#[test]
fn icons_taskbar_reserves_each_edge_and_preserves_auto_hide_and_style_switching() {
    let mut server = DisplayServer::new(DisplayConfig { width_px: 1000, height_px: 700, native_window_manager: true, ..DisplayConfig::default() });
    assert!(server.set_taskbar_style(2));
    assert!(server.set_dock_height(44));
    for (edge, expected) in [(0, (0, 656, 1000, 44)), (1, (0, 0, 1000, 44)), (2, (0, 0, 44, 700)), (3, (956, 0, 44, 700))] {
        assert!(server.set_taskbar_configuration(true, edge, false));
        assert_eq!(server.wm_taskbar_rect(), expected);
        assert_eq!(server.wm_reserved_taskbar_height(), if edge < 2 { 44 } else { 0 });
        assert_eq!(server.wm_reserved_taskbar_width(), if edge < 2 { 0 } else { 44 });
        server.inject_pointer_motion(500, 350, 0);
        assert!(server.set_taskbar_configuration(true, edge, true));
        assert_eq!(server.wm_reserved_taskbar_height(), 0);
        assert_eq!(server.wm_reserved_taskbar_width(), 0);
        assert!(!server.windows[&WM_TASKBAR.0].mapped);
    }
    assert!(server.set_taskbar_configuration(true, 0, false));
    assert!(server.set_dock_height(64));
    assert_eq!(server.wm_taskbar_rect(), (0, 636, 1000, 64));
    assert!(server.set_taskbar_style(1));
    assert_eq!(server.wm_reserved_taskbar_height(), 0);
    assert!(server.wm_taskbar_rect().2 < 1000);
    assert!(server.set_taskbar_style(0));
    assert_eq!(server.wm_taskbar_rect(), (0, 668, 1000, 32));
    assert!(!server.set_taskbar_style(3));
}

#[test]
fn icons_taskbar_groups_pinned_apps_paints_active_edges_and_keeps_tray_clickable() {
    let mut server = DisplayServer::new(DisplayConfig { width_px: 1200, height_px: 700, native_window_manager: true, ..DisplayConfig::default() });
    assert!(server.set_taskbar_style(2));
    let launcher = server.wm_quick_launchers[0].action;
    assert!(server.set_dock_identity(1, launcher, 123));
    for id in [7, 8] { assert!(server.set_dock_identity(0, id, 123)); }
    assert!(server.replace_native_tasks(vec![7,8].into_iter().map(|id| WmNativeTask {id,workspace:0,icon:1,minimized:false,maximized:false,active:id==7,title:format!("Document {id}").into_bytes()}).collect()));
    let target = WmChromeTarget::QuickLaunch(0);
    let targets = server.wm_dock_targets();
    assert!(!targets.iter().any(|target| matches!(target, WmChromeTarget::Task(_))), "pinned app owns its running group");
    let index = targets.iter().position(|item| *item == target).unwrap();
    let axis = index * server.wm_dock_step();
    let accent = server.wm_themes[usize::from(server.wm_theme)].taskbar[6] | 0xff00_0000;
    assert_eq!(server.surfaces[&WM_TASKBAR.0].pixel(axis as u16, 43), accent);
    assert!(!server.wm_text_overlays.iter().any(|row| row.window == WM_TASKBAR && row.text.starts_with(b"Document")));
    let (x,y,_,_) = server.wm_taskbar_rect();
    server.inject_pointer_motion(x + axis as i16 + 24, y + 22, 0);
    assert_eq!(server.wm_chrome_hover, Some(target));
    let icon_width = server.wm_text_overlays.iter().find(|row| row.window == WM_TASKBAR && row.text == b"__dock_icon:quick:0").unwrap().width;
    assert!(server.set_dock_magnification(true, 100, 100));
    assert_eq!(server.wm_text_overlays.iter().find(|row| row.window == WM_TASKBAR && row.text == b"__dock_icon:quick:0").unwrap().width, icon_width);
    for edge in 0..4 {
        server.set_taskbar_configuration(true, edge, false);
        let (x,y,w,h) = server.wm_taskbar_rect();
        let (index, item, start, extent) = if edge < 2 { server.wm_tray_layout() } else { server.wm_vertical_tray_layout() }.into_iter().find(|item| item.1 == TrayItem::Clock).unwrap();
        assert_eq!(item, TrayItem::Clock);
        let (px,py) = if edge < 2 { (start + extent/2, usize::from(h)/2) } else { (usize::from(w)/2, start + extent/2) };
        assert_eq!(server.wm_chrome_hit(WM_TASKBAR, x + px as i16, y + py as i16), Some(WmChromeTarget::Tray(index as u8)));
        assert!(server.wm_text_overlays.iter().any(|row| row.window == WM_TASKBAR && row.text.contains(&b':')), "clock remains painted");
    }
}

#[test]
fn icons_taskbar_placement_moves_paint_hits_and_launcher_together_without_covering_tray() {
    let mut server = DisplayServer::new(DisplayConfig { width_px: 1600, height_px: 1000, native_window_manager: true, ..DisplayConfig::default() });
    assert!(server.set_taskbar_style(2));
    for edge in 0..4 {
        assert!(server.set_taskbar_configuration(true, edge, false));
        let mut offsets = Vec::new();
        for placement in 0..3 {
            assert!(server.set_dock_icon_placement(placement));
            let offset = server.wm_dock_axis_padding();
            offsets.push(offset);
            let extent = server.wm_dock_targets().len() * server.wm_dock_step();
            assert!(offset + extent <= server.wm_icons_taskbar_available_axis());
            let vertical = server.wm_taskbar_edge.vertical();
            let icon = server.wm_text_overlays.iter().find(|row| row.window == WM_TASKBAR && row.text == b"__dock_icon:logo:0").unwrap();
            assert!(usize::from(if vertical { icon.y } else { icon.x }) >= offset);
            let (x,y,_,_) = server.wm_taskbar_rect();
            let (px,py) = if vertical { (x+22,y+offset as i16+24) } else { (x+offset as i16+24,y+22) };
            assert_eq!(server.wm_chrome_hit(WM_TASKBAR,px,py),Some(WmChromeTarget::Start));
            if offset > 24 {
                let (px,py) = if vertical { (x+22,y+10) } else { (x+10,y+22) };
                assert_eq!(server.wm_chrome_hit(WM_TASKBAR,px,py),None);
            }
            if placement == 1 {
                let axis = if vertical { 1000 } else { 1600 };
                assert_eq!(offset,(axis-extent)/2);
            }
            let (index,_,start,extent) = if vertical { server.wm_vertical_tray_layout() } else { server.wm_tray_layout() }.into_iter().find(|r|r.1==TrayItem::Clock).unwrap();
            let (px,py) = if vertical {(x+22,y+(start+extent/2) as i16)} else {(x+(start+extent/2) as i16,y+22)};
            assert_eq!(server.wm_chrome_hit(WM_TASKBAR,px,py),Some(WmChromeTarget::Tray(index as u8)));
        }
        assert_eq!(offsets[0],0);
        assert!(offsets[0]<offsets[1] && offsets[1]<=offsets[2]);
    }
    assert!(!server.set_dock_icon_placement(3));
}

#[test]
fn taskbar_translucency_changes_background_only_without_glass() {
    for style in 0..=2 {
        for edge in 0..=3 {
            let mut server = DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
            assert!(!server.wm_themes[usize::from(server.wm_theme)].backdrop_material);
            assert!(server.set_taskbar_style(style));
            assert!(server.set_taskbar_configuration(true, edge, false));
            assert!(server.replace_native_tasks(vec![WmNativeTask {id:7,workspace:0,icon:1,minimized:false,maximized:false,active:true,title:b"Document".to_vec()}]));
            for opacity in [72, 0, 100] {
                assert!(server.set_taskbar_opacity(Some(opacity)));
                let surface = &server.surfaces[&WM_TASKBAR.0];
                let alpha = u32::from(opacity) * 255 / 100;
                assert!(surface.pixels.as_ref().unwrap().iter().any(|pixel| *pixel >> 24 == alpha), "background {style}/{edge}/{opacity}");
                assert!(surface.pixels.as_ref().unwrap().iter().any(|pixel| *pixel >> 24 == 255), "foreground {style}/{edge}/{opacity}");
            }
            assert!(!server.set_taskbar_opacity(Some(101)));
            assert!(server.set_taskbar_opacity(None));
            if style != 1 {
                assert!(server.surfaces[&WM_TASKBAR.0].pixels.as_ref().unwrap().iter().all(|pixel| *pixel == 0 || *pixel >> 24 == 255));
            }
        }
    }
}

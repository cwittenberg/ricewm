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

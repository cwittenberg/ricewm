use super::*;

fn desktop(width:u16,height:u16)->DisplayServer {
    DisplayServer::new(DisplayConfig {width_px:width,height_px:height,native_window_manager:true,..DisplayConfig::default()})
}

#[test]
fn motif_menu_attaches_to_each_taskbar_edge_with_matching_pointer_geometry() {
    for edge in [WmTaskbarEdge::Bottom,WmTaskbarEdge::Top,WmTaskbarEdge::Left,WmTaskbarEdge::Right] {
        let mut server=desktop(1440,900);
        server.wm_themes[0].chrome_profile=WmChromeProfile::SplitControls;
        server.set_taskbar_configuration(true,edge.wire() as u8,false);
        server.inject_key(true,133,0);server.inject_key(false,133,0);
        let menu=&server.windows[&WM_MENU.0];
        let (x,y,w,h)=server.wm_taskbar_rect();
        match edge {
            WmTaskbarEdge::Bottom=>assert_eq!(i32::from(menu.y)+i32::from(menu.height),i32::from(y)),
            WmTaskbarEdge::Top=>assert_eq!(i32::from(menu.y),i32::from(y)+i32::from(h)),
            WmTaskbarEdge::Left=>assert_eq!(i32::from(menu.x),i32::from(x)+i32::from(w)),
            WmTaskbarEdge::Right=>assert_eq!(i32::from(menu.x)+i32::from(menu.width),i32::from(x)),
        }
        let rows=server.wm_menu_columns()[0].clone();
        let (rx,ry,rw,rh)=server.wm_theme_menu_row_rects(0,&rows,server.wm_menu_column_widths()[0])[0];
        let hit=server.wm_menu_hit(menu.x+(rx+rw/2) as i16,
            menu.y+(server.wm_menu_column_offsets()[0]+ry+rh/2) as i16).unwrap();
        assert_eq!(hit.2,rows[0]);
    }
}

#[test]
fn shell_theme_geometry_and_native_actions_share_rectangles_and_reset() {
    for edge in [WmTaskbarEdge::Bottom,WmTaskbarEdge::Top,WmTaskbarEdge::Left,WmTaskbarEdge::Right] {
        let mut server=desktop(1440,900);
        server.set_taskbar_configuration(true,edge.wire() as u8,false);
        let original_width=server.wm_start_button_width();
        assert!(server.set_theme_shell_layout([96,420,200,24,60,36,2]));
        assert!(server.set_theme_menu_row_height(30));
        assert_eq!(server.wm_start_button_width(),96);
        server.inject_key(true,133,0);server.inject_key(false,133,0);
        let root=server.windows[&WM_MENU.0].clone();
        let rows=server.wm_menu_columns()[0].clone();
        let x=server.wm_menu_column_x_offsets()[0];
        let y=server.wm_menu_column_offsets()[0];
        let rects=server.wm_theme_menu_row_rects(0,&rows,420);
        assert!(rects.iter().any(|r|r.0>24));
        for (index,(rx,ry,w,h)) in rects.iter().copied().enumerate() {
            let hit=server.wm_menu_hit(root.x+(x+rx+w/2) as i16,root.y+(y+ry+h/2) as i16);
            if rows[index] == WmMenuRow::Divider {
                assert_eq!(hit.unwrap().2, WmMenuRow::Divider);
                server.inject_pointer_button(true,1,root.x+(x+rx+w/2) as i16,root.y+(y+ry+h/2) as i16,0);
                assert_eq!(server.drain_wm_action(),None,"separators must never launch a command");
                assert!(server.windows[&WM_MENU.0].mapped);
                continue;
            }
            let hit=hit.unwrap();
            assert_eq!((hit.0,hit.1),(0,index));
        }
        for (rx,ry) in [(x+4,y+70),(x+50,y+20),(x+50,y+server.wm_theme_menu_panel_height(0,&rows)-10)] {
            assert!(server.wm_menu_hit(root.x+rx as i16,root.y+ry as i16).is_none(),"decorative band must not launch an action");
        }
        let run=rows.iter().position(|r|*r==WmMenuRow::Run).unwrap();
        let (rx,ry,w,h)=rects[run];
        server.inject_pointer_button(true,1,root.x+(x+rx+w/2) as i16,root.y+(y+ry+h/2) as i16,0);
        assert_eq!(server.drain_wm_action(),Some(WM_ACTION_RUN));
        assert!(server.set_theme_shell_layout([0,0,0,0,0,0,1]));
        assert_eq!(server.wm_start_button_width(),original_width);
    }
}

#[test]
fn shell_theme_rejects_invalid_tuple_atomically_and_compact_outputs_keep_single_column() {
    let mut server=desktop(390,844);
    assert!(server.set_theme_shell_layout([96,420,200,24,60,36,2]));
    assert!(!server.set_theme_shell_layout([999,420,200,24,60,36,2]));
    assert_eq!(server.wm_theme_shell_layout,[96,420,200,24,60,36,2]);
    let rows=server.wm_root_rows();
    let rects=server.wm_theme_menu_row_rects(0,&rows,380);
    assert!(rects.iter().all(|r|r.0==24));
    assert!(server.set_menu_layout(true,true,true));
    assert_eq!(server.wm_theme_menu_insets(0),(24,0,0),"applications-only panels have no root bands");
}

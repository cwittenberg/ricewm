use super::*;

fn beveled()->[u32;15]{[5,6,6,6,30,24,24,0,0,1,0,0,25,3,0x214]}

#[test]
fn beveled_frame_geometry_controls_and_metadata_share_the_client_bounds(){
    let mut server=DisplayServer::new(DisplayConfig{native_window_manager:true,..DisplayConfig::default()});
    assert!(server.set_window_chrome(beveled()));
    let owner=server.open_client().unwrap();let order=ClientByteOrder::LittleEndian;
    server.feed(owner,&setup_request(order,&[],&[])).unwrap();drain_all(&mut server,owner);
    let client=ResourceId(0x0020_0080);
    server.feed(owner,&create_window_request(order,client.0,STRUCTURE_NOTIFY_MASK)).unwrap();
    server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
    let frame=server.managed_frames[&client.0];let state=server.windows[&frame.0].clone();
    assert_eq!(server.managed_frame_insets(client),(6,6,30,6));
    assert_eq!((server.windows[&client.0].x,server.windows[&client.0].y),(6,30));
    assert_eq!((state.width,state.height),(332,236));
    for (x,action) in [(18,3),(state.width as i16-42,0),(state.width as i16-18,1)]{
        assert_eq!(server.wm_chrome_hit(frame,state.x+x,state.y+17),Some(WmChromeTarget::FrameControl(frame,action)));
    }
    assert_eq!(server.custom_frame_resize_edges(state.width,state.height,20,1),WM_RESIZE_LEFT|WM_RESIZE_TOP);
    assert_eq!(server.custom_frame_resize_edges(state.width,state.height,50,1),WM_RESIZE_TOP);
    assert!(server.wm_text_overlays.iter().any(|o|o.window==frame&&o.align==4&&o.width==0));
    server.inject_pointer_button(true,1,state.x+18,state.y+17,0);server.inject_pointer_button(false,1,state.x+18,state.y+17,0);
    assert!(server.windows[&WM_MENU.0].mapped,"the left bar opens the ordinary window menu");
    server.dismiss_wm_menu();
    assert!(server.set_window_chrome([0;15]));
    assert_eq!(server.managed_frame_insets(client),(WM_BORDER_WIDTH,WM_BORDER_WIDTH,WM_TITLE_HEIGHT,WM_BORDER_WIDTH));
    assert_eq!((server.windows[&client.0].width,server.windows[&client.0].height),(320,200));
}

#[test]
fn custom_frame_schema_rejects_invalid_geometry_and_duplicate_actions(){
    let mut server=DisplayServer::new(DisplayConfig::default());
    let mut bad=beveled();bad[0]=33;assert!(!server.set_window_chrome(bad));
    bad=beveled();bad[14]=0x114;assert!(!server.set_window_chrome(bad));
    bad=beveled();bad[14]=0x215;assert!(!server.set_window_chrome(bad));
    bad=beveled();bad[13]=0;assert!(!server.set_window_chrome(bad));
}

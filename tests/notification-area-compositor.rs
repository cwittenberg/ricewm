    use super::*;
    #[test]
    fn notification_icon_real_pixels_pointer_keyboard_and_teardown() {
        let mut server=DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        assert!(server.update_notification_icon(41,b"Actual application".to_vec(),2,1,vec![0xffff0000,0xff00ff00],false));
        let (_,x,y)=server.notification_icon_layout()[0];
        assert_eq!(server.surfaces[&WM_TASKBAR.0].pixel((x+2) as u16,(y+2) as u16),0xffff0000);
        let (tx,ty,_,_)=server.wm_taskbar_rect();let px=tx+x as i16+4;let py=ty+y as i16+4;
        server.inject_pointer_motion(px,py,0);
        assert_eq!(server.wm_quick_launch_tooltip(),Some(b"Actual application".as_slice()));
        server.inject_pointer_button(true,1,px,py,0);server.inject_pointer_button(false,1,px,py,0);
        assert_eq!(server.drain_notification_event(),Some([41,0x201,px as u32,py as u32]));
        assert_eq!(server.drain_notification_event(),Some([41,0x202,px as u32,py as u32]));
        assert!(server.focus_notification_icon(41));server.inject_key(true,36,0);
        assert_eq!(server.drain_notification_event().unwrap()[1],0x401);
        server.wm_dock_style=true; server.refresh_taskbar_geometry(); server.paint_wm_chrome(WM_TASKBAR);
        let (_,dx,dy)=server.notification_icon_layout()[0];let (ox,oy,_,_)=server.wm_taskbar_rect();
        assert_eq!(server.notification_icon_at(ox+dx as i16+4,oy+dy as i16+4),Some(41));
        assert_eq!(server.surfaces[&WM_TASKBAR.0].pixel((dx+2) as u16,(dy+2) as u16),0xffff0000);
        assert!(server.remove_notification_icon(41));assert!(server.notification_icon_layout().is_empty());assert!(server.wm_notification_focus.is_none());
    }

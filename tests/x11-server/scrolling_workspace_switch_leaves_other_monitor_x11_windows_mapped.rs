use super::*;


    #[test]
    fn scrolling_workspace_switch_leaves_other_monitor_x11_windows_mapped() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let left=ResourceId(0x0020_0190);let right=ResourceId(0x0020_0191);
        for (client,x) in [(left,30),(right,850)] {
            s.feed(owner,&create_window_request(order,client.0,0)).unwrap();
            s.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
            let frame=s.managed_frames[&client.0];s.apply_managed_geometry(client,frame,x,50,300,300);
        }
        s.set_scrolling_workspaces(true);s.switch_workspace(1);
        assert!(!s.windows[&s.managed_frames[&left.0].0].mapped);
        assert!(s.windows[&s.managed_frames[&right.0].0].mapped);
        assert!(s.select_shell_output(900,100));assert_eq!(s.current_workspace(),0);
        s.activate_managed_window(right);
        assert!(s.windows[&s.managed_frames[&right.0].0].mapped);
        assert_eq!(s.output_workspace(0),1);
    }

    #[test]
    fn x11_monitor_return_preserves_geometry_and_honors_explicit_moves() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();let _=drain_all(&mut s,owner);
        let client=ResourceId(0x0020_0199);s.feed(owner,&create_window_request(order,client.0,STRUCTURE_NOTIFY_MASK)).unwrap();s.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame=s.managed_frames[&client.0];assert!(s.move_overview_window(frame.0,0,900,100));
        let original=(s.windows[&client.0].width,s.windows[&client.0].height);
        let identity:Vec<u32>=(0..16).collect();let mut swapped=identity.clone();swapped.swap(0,1);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&identity,0));assert_eq!(s.windows[&frame.0].x,100);assert_eq!(s.x11_output_homes.len(),1);
        assert!(s.map_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)],&swapped,0));
        assert_eq!(s.windows[&frame.0].x,100);assert!(s.x11_output_homes.is_empty());assert_eq!((s.windows[&client.0].width,s.windows[&client.0].height),original);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&swapped,0));assert_eq!(s.x11_output_homes.len(),1);
        assert!(s.move_overview_window(frame.0,0,240,120));assert!(s.x11_output_homes.is_empty());
        assert!(s.map_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)],&identity,0));assert_eq!(s.windows[&frame.0].x,240,"explicit fallback placement wins over reconnect");
        assert!(s.move_overview_window(frame.0,0,900,100));s.toggle_fullscreen(client,1);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&identity,0));assert_eq!(s.windows[&frame.0].width,800);
        assert!(s.map_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)],&identity,0));assert_eq!(s.windows[&frame.0].x,800);assert_eq!(s.windows[&frame.0].width,800);
        s.toggle_fullscreen(client,0);assert_eq!(s.windows[&frame.0].x,900);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&identity,0));assert_eq!(s.x11_output_homes.len(),1);
        s.close_client(owner);assert!(s.x11_output_homes.is_empty());
    }

    #[test]
    fn x11_simultaneous_monitor_removal_and_resized_reordered_return() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:2400,height_px:900,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(2400,900,vec![(0,0,800,700),(800,100,800,800),(1600,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let clients=[ResourceId(0x0020_0195),ResourceId(0x0020_0196),ResourceId(0x0020_0197)];
        for (index,&client) in clients.iter().enumerate(){
            s.feed(owner,&create_window_request(order,client.0,0)).unwrap();
            s.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
            let frame=s.managed_frames[&client.0];
            s.apply_managed_geometry(client,frame,100+index as i16*800,if index==1{200}else{100},300,300);
        }
        s.toggle_fullscreen(clients[2],1);
        let identity:Vec<u32>=(0..16).collect();
        assert!(s.map_output_layout(900,750,vec![(0,0,900,750)],&identity,0));
        assert_eq!(s.x11_output_homes.len(),2);
        assert!(s.map_output_layout(1000,800,vec![(0,0,1000,800)],&identity,0));
        assert_eq!(s.x11_output_homes.len(),2,"fallback resizing retains both original homes");
        let mut reorder=identity.clone();reorder[0]=1;reorder[1]=2;reorder[2]=0;
        assert!(s.map_output_layout(2700,950,vec![(0,150,900,800),(900,0,800,750),(1700,50,1000,900)],&reorder,0));
        assert!(s.x11_output_homes.is_empty());
        for (index,&client) in clients.iter().enumerate(){
            let frame=&s.windows[&s.managed_frames[&client.0].0];
            assert_eq!((frame.x,frame.y),[(1000,100),(1800,150),(0,150)][index]);
        }
        let full=&s.windows[&s.managed_frames[&clients[2].0].0];
        assert_eq!((full.width,full.height),(900,800));
        s.toggle_fullscreen(clients[2],0);
        let restored=&s.windows[&s.managed_frames[&clients[2].0].0];
        assert_eq!((restored.x,restored.y),(100,250));
        for client in clients{assert_eq!((s.windows[&client.0].width,s.windows[&client.0].height),(300,300));}
    }

    #[test]
    fn scrolling_x11_columns_keep_preferred_widths_when_returning_to_a_reordered_monitor() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();let _=drain_all(&mut s,owner);
        let clients=[ResourceId(0x0020_019a),ResourceId(0x0020_019b)];
        for (index,&client) in clients.iter().enumerate(){s.feed(owner,&create_window_request(order,client.0,0)).unwrap();s.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();let frame=s.managed_frames[&client.0];assert!(s.move_overview_window(frame.0,0,100+index as i32*800,100));}
        let apply=|s:&mut DisplayServer|{s.begin_auto_tile(true);s.set_auto_tile_scrolling(true);s.apply_auto_tile();};
        s.activate_managed_window(clients[1]);apply(&mut s);assert!(s.resize_tile(1));apply(&mut s);
        let widths:Vec<_>=clients.iter().map(|client|s.windows[&s.managed_frames[&client.0].0].width).collect();assert_ne!(widths[0],widths[1]);
        let identity:Vec<u32>=(0..16).collect();let mut swapped=identity.clone();swapped.swap(0,1);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&identity,0));apply(&mut s);
        let mut invalid=swapped.clone();invalid[2]=invalid[0];assert!(!s.map_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)],&invalid,0));
        assert!(s.map_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)],&swapped,0));apply(&mut s);
        for (index,&client) in clients.iter().enumerate(){let window=&s.windows[&s.managed_frames[&client.0].0];assert_eq!(window.width,widths[index]);let clip=s.tile_layouts[&0].strip.clip(WmTaskTarget::X11(client)).unwrap();assert_eq!(clip.x,if index==0{800}else{0});}
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&swapped,0));apply(&mut s);assert_eq!(s.x11_output_homes.len(),1);
        assert!(s.insert_tile(0,clients[1].0,0,clients[0].0,3));assert!(s.x11_output_homes.is_empty(),"an explicit insertion into the fallback strip overrides return");
    }

    #[test]
    fn held_drag_can_select_destination_monitor_without_releasing_capture() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let source=ResourceId(0x0020_0190);s.feed(owner,&create_window_request(order,source.0,0)).unwrap();
        s.replace_native_tasks(vec![WmNativeTask{id:11,workspace:1,icon:0,minimized:false,maximized:false,active:false,title:b"Right".to_vec()}]);
        s.set_scrolling_workspaces(true);
        assert!(s.select_shell_output(900,100));s.switch_workspace(1);
        assert!(s.select_shell_output(100,100));
        assert!(!s.select_drag_output(source.0,900,100));
        s.pointer_grab=Some(ActivePointerGrab{owner:owner.0,window:source,owner_events:true,event_mask:0,automatic:false});
        assert!(!s.select_drag_output(source.0,900,100));
        s.input_state=0x0100;
        assert!(!s.select_shell_output(900,100));
        assert!(!s.select_drag_output(0,900,100));assert!(!s.select_drag_output(source.0,-1,100));
        assert_eq!(s.active_output(),0);
        assert!(s.select_drag_output(source.0,900,100));assert_eq!(s.current_workspace(),1);
        s.switch_workspace(2);
        assert_eq!((s.output_workspace(0),s.output_workspace(1)),(0,2));
        assert_eq!(s.pointer_grab.unwrap().owner,owner.0);assert_eq!(s.input_state,0x0100);
        assert!(s.select_drag_output(source.0,900,100),"selection of the same destination succeeds");
        s.input_state=0;assert!(!s.select_drag_output(source.0,100,100));assert_eq!(s.active_output(),1);
    }

    #[test]
    fn incoming_drag_scroll_requires_held_grab_owner_and_preserves_capture() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let first=ResourceId(0x0020_0190);
        for id in first.0..first.0+4{s.feed(owner,&create_window_request(order,id,0)).unwrap();s.feed(owner,&request(order,8,0,&id.to_le_bytes())).unwrap();}
        s.activate_managed_window(first);s.begin_auto_tile(true);s.set_auto_tile_scrolling(true);s.apply_auto_tile();
        let foreign=s.open_client().unwrap();s.feed(foreign,&setup_request(order,&[],&[])).unwrap();
        let other=ResourceId(0x0040_0190);s.feed(foreign,&create_window_request(order,other.0,0)).unwrap();
        assert!(!s.scroll_drag_tiles_at(first.0,100,400,300));
        s.pointer_grab=Some(ActivePointerGrab{owner:owner.0,window:first,owner_events:true,event_mask:0,automatic:false});
        assert!(!s.scroll_drag_tiles_at(first.0,100,400,300),"a stale grab without the held button is insufficient");
        s.input_state=0x0100;
        s.capture_workspace_layout(0);let before=s.tile_snapshot_buffer.clone();
        for (source,delta,x,y) in [(other.0,100,400,300),(0,100,400,300),(ROOT_WINDOW.0,100,400,300),(first.0,i32::MIN,400,300),(first.0,4097,400,300),(first.0,100,i32::MAX,300)]{
            assert!(!s.scroll_drag_tiles_at(source,delta,x,y));
        }
        s.keyboard_grab=Some(ActiveKeyboardGrab{owner:foreign.0,window:other,owner_events:true});
        assert!(!s.scroll_drag_tiles_at(first.0,100,400,300));s.keyboard_grab=None;
        s.capture_workspace_layout(0);assert_eq!(s.tile_snapshot_buffer,before,"rejected navigation is atomic");
        assert!(!s.scroll_tiles_at(100,400,300),"ordinary tiling still respects the grab");
        let modal=s.atoms.intern("_NET_WM_STATE_MODAL",false);
        s.set_wm_property(first,"_NET_WM_STATE","ATOM",32,modal.0.to_le_bytes().to_vec());
        assert!(!s.scroll_drag_tiles_at(first.0,100,400,300),"modal windows still block edge navigation");
        s.set_wm_property(first,"_NET_WM_STATE","ATOM",32,Vec::new());
        s.keyboard_grab=Some(ActiveKeyboardGrab{owner:owner.0,window:first,owner_events:true});
        assert!(s.scroll_drag_tiles_at(first.0,100,400,300));
        let grab=s.pointer_grab.unwrap();assert_eq!(grab.owner,owner.0);assert_eq!(grab.window,first);assert!(!grab.automatic);assert_eq!(s.input_state,0x0100);
        s.input_state=0;assert!(!s.scroll_drag_tiles_at(first.0,100,400,300));
    }

    #[test]
    fn incoming_drag_scroll_uses_pointer_monitor_tiling_state() {
        let order=ClientByteOrder::LittleEndian;
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        assert!(s.set_output_layout(1600,700,vec![(0,0,800,700),(800,0,800,700)]));
        let owner=s.open_client().unwrap();s.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let source=ResourceId(0x0020_0190);
        s.feed(owner,&create_window_request(order,source.0,0)).unwrap();
        s.feed(owner,&request(order,8,0,&source.0.to_le_bytes())).unwrap();
        s.set_scrolling_workspaces(true);
        assert!(s.select_shell_output(900,100));s.switch_workspace(1);
        s.replace_native_tasks((11..15).map(|id|WmNativeTask{id,workspace:1,icon:0,minimized:false,maximized:false,active:false,title:b"Right monitor".to_vec()}).collect());
        s.begin_auto_tile(true);s.set_auto_tile_workspace(1);s.set_auto_tile_scrolling(true);
        for id in 11..15{s.add_auto_tile_native(id);s.set_auto_tile_position(id,900,200);}
        s.apply_auto_tile();
        assert!(s.select_shell_output(100,100));
        s.begin_auto_tile(false);s.set_auto_tile_workspace(0);s.apply_auto_tile();
        assert_eq!((s.output_workspace(0),s.output_workspace(1)),(0,1));
        assert!(s.select_shell_output(900,100));
        assert!(s.scroll_tiles_at(100,1200,300),"ordinary pointer panning uses the selected workspace's committed state");
        assert!(s.scroll_tiles(-100),"keyboard panning is independent of another monitor's last plan");
        assert!(s.select_shell_output(100,100));
        assert!(!s.scroll_tiles_at(100,100,300));assert!(!s.scroll_tiles(100));
        s.pointer_grab=Some(ActivePointerGrab{owner:owner.0,window:source,owner_events:true,event_mask:0,automatic:false});
        s.input_state=0x0100;
        let area=TileRect{x:800,y:0,width:800,height:700};
        let before=s.tile_layouts[&1].rectangles(area);
        assert!(!s.scroll_drag_tiles_at(source.0,100,100,300),"the floating monitor must not scroll");
        assert!(s.scroll_drag_tiles_at(source.0,100,1200,300),"the pointer monitor remains scrollable when another monitor was last laid out floating");
        let after=s.tile_layouts[&1].rectangles(area);
        assert_eq!(before.len(),4);
        for ((id,old),(new_id,new)) in before.iter().zip(&after){
            assert_eq!(id,new_id);assert_eq!(new.x,old.x-100);
            assert_eq!((new.y,new.width,new.height),(old.y,old.width,old.height));
        }
        assert_eq!((s.output_workspace(0),s.output_workspace(1),s.current_workspace(),s.active_output()),(0,1,0,0),"edge navigation does not steal monitor focus or change workspaces");
        assert_eq!(s.pointer_grab.unwrap().owner,owner.0);
        s.begin_auto_tile(false);s.set_auto_tile_workspace(1);s.apply_auto_tile();
        assert!(!s.scroll_drag_tiles_at(source.0,100,1200,300),"a saved scrolling layout is insufficient after tiling is disabled");
    }

    #[test]
    fn scrolling_x11_fullscreen_stays_in_strip_and_restores_width() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig{width_px:1000,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        server.replace_native_tasks(vec![WmNativeTask{id:12,workspace:0,icon:0,minimized:false,maximized:false,active:false,title:b"Native".to_vec()}]);
        server.activate_managed_window(client);
        let apply=|s:&mut DisplayServer|{s.begin_auto_tile(true);s.set_auto_tile_scrolling(true);s.add_auto_tile_native(12);s.apply_auto_tile();};
        apply(&mut server);let frame=server.managed_frames[&client.0];
        let width=server.windows[&frame.0].width;
        server.toggle_fullscreen(client,1);apply(&mut server);
        let full=&server.windows[&frame.0];
        assert_eq!((full.x,full.y,full.width,full.height),(0,0,1000,700));
        assert!(server.auto_tile_rects[0].1>=1000);
        server.toggle_fullscreen(client,0);apply(&mut server);
        assert_eq!(server.windows[&frame.0].width,width);
    }

    #[test]
    fn directional_focus_crosses_native_and_x11_and_excludes_hidden_tasks() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame=server.managed_frames[&client.0];
        server.apply_managed_geometry(client,frame,20,20,100,100);
        server.replace_native_tasks(vec![WmNativeTask{id:12,workspace:0,icon:0,minimized:false,maximized:false,active:false,title:b"Native".to_vec(),
        }]);
        server.navigation_begin();
        server.navigation_native_frame(12,300,20,100,100);
        server.activate_managed_window(client);
        assert_eq!(server.focus_direction(1),12);
        assert_eq!(server.focus_direction(0),0,"no wrapping at the edge");
        server.native_tasks[0].active=true;
        assert_eq!(server.focus_direction(0),65536);
        assert_eq!(server.managed_client_ancestor(server.focus_window),Some(client));
        server.native_tasks[0].active=false;
        server.native_tasks[0].minimized=true;
        assert_eq!(server.focus_direction(1),0);
        server.native_tasks[0].minimized=false;
        server.native_tasks[0].workspace=1;
        assert_eq!(server.focus_direction(1),0);
        assert_eq!(server.focus_direction(4),0);
        server.native_tasks[0].workspace=0;
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.apply_auto_tile();
        let native=server.auto_tile_rects[0];
        server.navigation_native_frame(12,i32::from(native.1),i32::from(native.2),u32::from(native.3),u32::from(native.4),
        );
        assert!(!server.tile_control_available(0,client.0,0));
        assert!(server.tile_control_available(0,client.0,1));
        assert!(server.tile_control_available(1,12,0));
        assert!(!server.tile_control_available(1,12,1));
        assert!(!server.tile_control_available(1,12,2));
        assert!(server.tile_control_available(0,client.0,4));
        assert!(server.swap_direction(1));
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.apply_auto_tile();
        let swapped=server.auto_tile_rects[0];
        assert!(swapped.1+i32::from(swapped.3)<=i32::from(server.windows[&frame.0].x));
        assert_eq!(server.managed_client_ancestor(server.focus_window),Some(client),"swap keeps focus on the moving app");
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.apply_auto_tile();
        assert_eq!(server.auto_tile_rects[0],swapped,"reflow preserves swapped order");
        let before_width=server.windows[&frame.0].width;
        assert!(server.resize_tile(0));
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.apply_auto_tile();
        assert_eq!(server.windows[&frame.0].width,before_width+32);
        assert_eq!(server.auto_tile_rects[0].3,swapped.3-32);
        server.native_tasks[0].active=true;
        assert!(server.resize_tile(0),"native and X11 resizing share the divider");
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.apply_auto_tile();
        assert_eq!(server.windows[&frame.0].width,before_width);
        assert_eq!(server.auto_tile_rects[0],swapped);
        server.begin_auto_tile(false);
        assert!(!server.swap_direction(0),"floating layouts cannot accidentally mutate tile order");
        assert!(!server.resize_tile(0));
        server.apply_auto_tile();
        assert_eq!(server.windows[&frame.0].x,20,"swap does not replace the saved floating position");
    }

    #[test]
    fn layout_snapshot_restores_remapped_members_and_rejects_invalid_state_atomically() {
        let mut server=DisplayServer::new(DisplayConfig{width_px:1000,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let tasks=|a,b|{
            vec![
            WmNativeTask{id:a,workspace:0,icon:0,minimized:false,maximized:false,active:true,title:b"First".to_vec(),
                },
            WmNativeTask{id:b,workspace:0,icon:0,minimized:false,maximized:false,active:false,title:b"Second".to_vec(),
                },
        ]};
        let apply=|server:&mut DisplayServer,a,b|{server.begin_auto_tile(true);server.add_auto_tile_native(a);server.add_auto_tile_native(b);server.apply_auto_tile();};
        server.replace_native_tasks(tasks(11,12));
        apply(&mut server,11,12);
        assert!(server.resize_tile(0));apply(&mut server,11,12);
        let expected=server.auto_tile_rects.clone();
        assert!(server.capture_workspace_layout(0)>0);
        let mut words=server.tile_snapshot_buffer.clone();
        server.replace_native_tasks(tasks(21,22));
        apply(&mut server,21,22);
        assert!(!server.restore_workspace_layout(0),"stale session ids cannot restore");
        for pair in words[2..6].chunks_exact_mut(2) {assert_eq!(pair[0],1);pair[1]+=10;}
        server.tile_snapshot_buffer=words.clone();
        assert!(server.restore_workspace_layout(0));apply(&mut server,21,22);
        let remapped:Vec<_>=expected.into_iter().map(|(id,x,y,w,h)|(id+10,x,y,w,h)).collect();
        assert_eq!(server.auto_tile_rects,remapped);
        server.tile_snapshot_buffer.push(99);
        assert!(!server.restore_workspace_layout(0));apply(&mut server,21,22);
        assert_eq!(server.auto_tile_rects,remapped,"failed import does not mutate the live tree");
        server.tile_snapshot_buffer=words;
        assert!(!server.restore_workspace_layout(1),"windows from another workspace are rejected");
    }

    #[test]
    fn application_minimum_hints_validate_flags_type_and_length() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig {width_px:1200,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let hints=|flags,width,height|{let mut words=vec![0u32;18];words[0]=flags;words[5]=width;words[6]=height;words[15]=700;words[16]=240;words.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()};
        let (left,right,top,bottom)=server.managed_frame_insets(client);
        let mut contradictory=hints(16|32,800,300);
        contradictory[28..32].copy_from_slice(&400u32.to_le_bytes());
        contradictory[32..36].copy_from_slice(&200u32.to_le_bytes());
        server.set_wm_property(client,"WM_NORMAL_HINTS","WM_SIZE_HINTS",32,contradictory.clone(),
        );
        assert_eq!(server.managed_client_size_limits(client),(800,300,800,300));
        contradictory[28..32].copy_from_slice(&0u32.to_le_bytes());
        contradictory[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        server.set_wm_property(client,"WM_NORMAL_HINTS","WM_SIZE_HINTS",32,contradictory,
        );
        assert_eq!(server.managed_client_size_limits(client),(800,300,u16::MAX,u16::MAX));
        server.set_wm_property(client,"WM_NORMAL_HINTS","WM_SIZE_HINTS",32,hints(16,800,300),
        );
        assert_eq!(server.managed_tile_minimum(client),(800+left+right+8,300+top+bottom+8));
        server.begin_auto_tile(true);server.add_auto_tile_native(12);server.set_auto_tile_native_minimum(12,200,100);server.apply_auto_tile();
        assert!(server.windows[&client.0].width>=800);
        let snapshot=server.capture_workspace_layout(0);assert!(snapshot>0);
        server.set_wm_property(client,"WM_NORMAL_HINTS","WM_SIZE_HINTS",32,hints(256,800,300),
        );
        assert_eq!(server.managed_tile_minimum(client),(700+left+right+8,240+top+bottom+8));
        for (kind,format,data) in [("STRING",32,hints(16,800,300)),("WM_SIZE_HINTS",8,hints(16,800,300)),("WM_SIZE_HINTS",32,vec![16;28]),("WM_SIZE_HINTS",32,hints(0,800,300)),
        ] {
            server.set_wm_property(client,"WM_NORMAL_HINTS",kind,format,data);
            assert_eq!(server.managed_tile_minimum(client),(0,0));
        }
    }

    #[test]
    fn automatic_tiling_shares_space_with_native_windows_and_restores_floating_geometry() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame=server.managed_frames[&client.0];
        let original=server.windows[&frame.0].clone();
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        assert_eq!(server.apply_auto_tile(),1);
        let native=server.auto_tile_rects[0];
        let tiled=server.windows[&frame.0].clone();
        assert!(i32::from(tiled.x)+i32::from(tiled.width)<=native.1);
        assert!(native.1+i32::from(native.3)<=800);
        assert!(native.2+i32::from(native.4)<=600-i32::from(WM_TASKBAR_HEIGHT));
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        server.add_auto_tile_native(13);
        assert_eq!(server.apply_auto_tile(),2);
        assert!(server.auto_tile_rects[0].2+i32::from(server.auto_tile_rects[0].4)<=server.auto_tile_rects[1].2);
        server.begin_auto_tile(true);
        server.add_auto_tile_native(12);
        assert_eq!(server.apply_auto_tile(),1);
        assert_eq!(server.auto_tile_rects[0],native);
        // A floating workspace must not restore a hidden tiled workspace.
        server.switch_workspace(1);
        server.begin_auto_tile(false);
        server.apply_auto_tile();
        assert_eq!(server.windows[&frame.0].width, tiled.width);
        assert!(server.auto_tile_restore.contains_key(&client.0));
        server.switch_workspace(0);
        server.begin_auto_tile(false);
        assert_eq!(server.apply_auto_tile(),0);
        let restored=&server.windows[&frame.0];
        assert_eq!((restored.x,restored.y,restored.width,restored.height),(original.x,original.y,original.width,original.height));
        server.begin_auto_tile(true);
        server.apply_auto_tile();
        assert!(server.auto_tile_restore.contains_key(&client.0));
        server.destroy_window_tree(client);
        assert!(!server.auto_tile_restore.contains_key(&client.0));
    }

    #[test]
    fn managed_initial_cascade_and_dynamic_workarea_keep_ordinary_frames_visible() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);

        let first = ResourceId(0x0020_0190);
        let second = ResourceId(0x0020_0191);
        for client in [first, second] {
            server
                .feed(
                    owner,
                    &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
                )
                .unwrap();
            let state = server.windows.get_mut(&client.0).unwrap();
            state.x = 0;
            state.y = 0;
            server
                .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
                .unwrap();
        }
        let _ = drain_all(&mut server, owner);

        let first_frame = server.managed_frames[&first.0];
        let second_frame = server.managed_frames[&second.0];
        let first_initial = server.windows[&first_frame.0].clone();
        let second_initial = server.windows[&second_frame.0].clone();
        assert_eq!((first_initial.x, first_initial.y), (24, 20));
        assert_eq!((second_initial.x, second_initial.y), (46, 38));
        assert_ne!(
            (first_initial.x, first_initial.y),
            (second_initial.x, second_initial.y),
            "default-position top levels must use RiceWM's managed cascade",
        );
        let bottom_panel_top = 600u16.saturating_sub(WM_TASKBAR_HEIGHT);
        for frame in [&first_initial, &second_initial] {
            assert!(frame.y >= 0);
            assert!(
                u16::try_from(frame.y)
                    .unwrap_or_default()
                    .saturating_add(frame.height)
                    <= bottom_panel_top,
                "initial managed frames must remain above the reserved bottom panel",
            );
        }

        // Model an already-open fixed-size window under a newly enabled top
        // panel and another ordinary frame overlapping the bottom panel.
        assert!(server.set_managed_window_resizable(first.0, false));
        server.windows.get_mut(&first_frame.0).unwrap().y = 0;
        server.windows.get_mut(&second_frame.0).unwrap().y = 590;
        assert!(server.update_gnome_panel(true, Vec::new()));

        let first_reflowed = server.windows[&first_frame.0].clone();
        let second_reflowed = server.windows[&second_frame.0].clone();
        assert_eq!(first_reflowed.y, i16::try_from(WM_TASKBAR_HEIGHT).unwrap());
        assert_eq!(first_reflowed.width, first_initial.width);
        assert_eq!(first_reflowed.height, first_initial.height);
        assert_eq!(second_reflowed.width, second_initial.width);
        assert_eq!(second_reflowed.height, second_initial.height);
        let (_, work_height) = server.wm_workarea();
        let work_bottom = WM_TASKBAR_HEIGHT.saturating_add(work_height);
        assert_eq!(
            u16::try_from(second_reflowed.y)
                .unwrap_or_default()
                .saturating_add(second_reflowed.height),
            work_bottom,
            "ordinary managed frames must be clamped above the bottom panel",
        );

        let configure = drain_all(&mut server, owner);
        for client in [first, second] {
            assert!(
                configure.chunks_exact(32).any(|event| {
                    event[0] == (22 | 0x80)
                        && u32::from_le_bytes(event[8..12].try_into().unwrap()) == client.0
                }),
                "dynamic work-area placement must reach each client through ConfigureNotify"
            );
        }

        assert!(server.configure_gnome_panel(48, false));
        assert_eq!(server.windows[&first_frame.0].y, 48);
        assert_eq!(server.windows[&first_frame.0].width, first_initial.width);
        assert_eq!(server.windows[&first_frame.0].height, first_initial.height);

        server.windows.get_mut(&first_frame.0).unwrap().x = 0;
        assert!(server.set_taskbar_configuration(true, WmTaskbarEdge::Left.wire() as u8, false,));
        let left_reflowed = &server.windows[&first_frame.0];
        assert_eq!(left_reflowed.x, i16::try_from(WM_TASKBAR_HEIGHT).unwrap());
        assert!(
            i32::from(left_reflowed.x) + i32::from(left_reflowed.width) <= 800,
            "ordinary frames must start beyond a left taskbar",
        );

        server.windows.get_mut(&second_frame.0).unwrap().x = 790;
        assert!(server.set_taskbar_configuration(true, WmTaskbarEdge::Right.wire() as u8, false,));
        let right_reflowed = &server.windows[&second_frame.0];
        assert!(
            i32::from(right_reflowed.x) + i32::from(right_reflowed.width)
                <= 800 - i32::from(WM_TASKBAR_HEIGHT),
            "ordinary frames must end before a right taskbar",
        );
    }

    #[test]
    fn floating_corner_resize_honors_size_limits_and_preserves_opposite_edges() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig{width_px:1200,height_px:800,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0186);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame=server.managed_frames[&client.0];
        let mut words=vec![0u32;18];words[0]=16|32;words[5]=400;words[6]=240;words[7]=700;words[8]=450;
        server.set_wm_property(client,"WM_NORMAL_HINTS","WM_SIZE_HINTS",32,words.into_iter().flat_map(u32::to_le_bytes).collect(),
        );
        for (left,top,grow) in [false,true].into_iter().flat_map(|grow|{
            [(false,false,grow),(true,false,grow),(false,true,grow),(true,true,grow),
            ]}) {
            server.apply_managed_geometry(client,frame,200,160,600,400);
            let before=server.windows[&frame.0].clone();
            let x=before.x+if left {2}else{before.width as i16-2};
            let y=before.y+if top {2}else{before.height as i16-2};
            server.inject_pointer_button(true,1,x,y,0);
            assert!(server.wm_drag.is_some(),"corner left={left} top={top}, hit={:?}",server.wm_chrome_hit(frame,x,y));
            let sign=if grow {-1}else{1};
            let next_x=x+sign*if left {500}else{-500};let next_y=y+sign*if top {350}else{-350};
            server.inject_pointer_motion(next_x,next_y,0x100);
            let after=server.windows[&frame.0].clone();
            assert_eq!((server.windows[&client.0].width,server.windows[&client.0].height),if grow {(700,450)}else{(400,240)});
            assert_eq!(if left {after.x as i32+i32::from(after.width)}else{after.x as i32},if left {before.x as i32+i32::from(before.width)}else{before.x as i32});
            assert_eq!(if top {after.y as i32+i32::from(after.height)}else{after.y as i32},if top {before.y as i32+i32::from(before.height)}else{before.y as i32});
            server.inject_pointer_button(false,1,next_x,next_y,0x100);
        }
        // Hints can tighten while an existing floating frame is larger. A
        // single-edge resize must also repair the other, now-invalid axis.
        server.apply_managed_geometry(client,frame,200,160,800,500);
        let before=server.windows[&frame.0].clone();
        let x=before.x+before.width as i16-2;let y=before.y+before.height as i16/2;
        server.inject_pointer_button(true,1,x,y,0);
        server.inject_pointer_motion(x+100,y,0x100);
        assert_eq!((server.windows[&client.0].width,server.windows[&client.0].height),(700,450));
        server.inject_pointer_button(false,1,x+100,y,0x100);
    }

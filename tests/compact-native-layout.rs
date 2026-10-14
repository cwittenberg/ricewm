use super::*;
use rice_native_app_protocol::UiPatch;

#[test]
fn compact_toolbar_reflows_controls_without_changing_desktop_layout() {
    let mut host = NativeUiHost::default();
    let app = AppId::new(1).unwrap();
    let window = WindowId::new(1).unwrap();
    let id = |n| WidgetId::new(n).unwrap();
    let mut patches = vec![UiPatch::Create {id:id(1),parent:None,index:0,kind:WidgetKind::ToolBar as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None}];
    for index in 0..12 {
        patches.push(UiPatch::Create {id:id(2+index),parent:Some(id(1)),index:index as u32,kind:WidgetKind::Button as u16,state:WidgetState::ENABLED.bits(),text:Some(format!("Action {index}")),icon:None});
    }
    host.apply_patch(&UiPatchEnvelope {app,window,sequence:1,patches}).unwrap();
    let constraints = Constraints::new(Size {width:320,height:400}, Size {width:320,height:400}).unwrap();
    let desktop = host.render(app,window,constraints).unwrap();
    let original = desktop.hit_targets.iter().map(|hit|(hit.widget,hit.rect)).collect::<Vec<_>>();
    host.compact_form_factor = true;
    let compact = host.render(app,window,constraints).unwrap();
    let controls = compact.hit_targets.iter().filter(|hit|hit.widget != id(1)).collect::<Vec<_>>();
    assert_eq!(controls.len(),12);
    assert!(controls.iter().any(|hit|hit.rect.origin.y != controls[0].rect.origin.y), "{controls:?}");
    for hit in controls {
        assert!(hit.rect.size.width>=32);
        assert!(hit.rect.origin.x>=0);
        assert!(hit.rect.origin.x as u32+hit.rect.size.width<=320);
        assert_eq!(host.hit_test(app,window,hit.rect.origin).unwrap().widget,hit.widget);
    }
    host.compact_form_factor = false;
    let restored = host.render(app,window,constraints).unwrap();
    assert_eq!(restored.hit_targets.iter().map(|hit|(hit.widget,hit.rect)).collect::<Vec<_>>(),original);
}

#[test]
fn compact_menu_flips_above_and_keeps_last_commands_reachable() {
    let mut host = NativeUiHost::default();
    let app = AppId::new(1).unwrap(); let window = WindowId::new(1).unwrap();
    let id = |n| WidgetId::new(n).unwrap();
    let mut patches = vec![UiPatch::Create {id:id(1),parent:None,index:0,kind:WidgetKind::Menu as u16,state:WidgetState::ENABLED.bits(),text:Some("Actions".into()),icon:None}];
    for index in 0..30 { patches.push(UiPatch::Create {id:id(2+index),parent:Some(id(1)),index:index as u32,kind:WidgetKind::MenuItem as u16,state:WidgetState::ENABLED.bits(),text:Some(format!("Command {index}")),icon:None}); }
    host.apply_patch(&UiPatchEnvelope {app,window,sequence:1,patches}).unwrap();
    let tree = &host.windows[&(app,window)].tree;
    let theme = host.theme();
    let _measurements = layout_measurements::RenderMeasurements::new(tree,theme,true);
    for scroll in [0,29] {
        let mut geometry = BTreeMap::new();
        layout::layout_menu_popup(tree,id(1),Size {width:320,height:200},Some(Point {x:310,y:190}),scroll,theme,&mut geometry);
        let popup = render::menu_popup_rect(tree,id(1),&geometry,theme).unwrap();
        assert!(popup.origin.x>=0 && popup.origin.y>=0);
        assert!(popup.origin.x as u32 + popup.size.width<=320);
        assert!(popup.origin.y as u32 + popup.size.height<=200);
        if scroll==29 { assert!(geometry.contains_key(&id(31)), "last command remains scrollable"); }
    }
}

#[test]
fn compact_navigation_opens_selects_and_restores_without_application_layout_changes() {
    let mut host = NativeUiHost::default(); host.compact_form_factor = true;
    let app = AppId::new(1).unwrap(); let window = WindowId::new(1).unwrap(); let id=|n|WidgetId::new(n).unwrap();
    let specs = [(1,None,0,WidgetKind::SplitPane,"split:250"),(2,Some(1),0,WidgetKind::ScrollView,"0/0"),(3,Some(2),0,WidgetKind::List,""),(4,Some(3),0,WidgetKind::Button,"Documents"),(5,Some(1),1,WidgetKind::Button,"File content")];
    let mut patches: Vec<_>=specs.into_iter().map(|(n,parent,index,kind,text)| UiPatch::Create{id:id(n),parent:parent.map(id),index,kind:kind as u16,state:WidgetState::ENABLED.bits(),text:Some(text.into()),icon:None}).collect();
    for index in 0..30 { patches.push(UiPatch::Create{id:id(10+index),parent:Some(id(3)),index:1+index as u32,kind:WidgetKind::Button as u16,state:WidgetState::ENABLED.bits(),text:Some(format!("Location {index}")),icon:None}); }
    host.apply_patch(&UiPatchEnvelope{app,window,sequence:1,patches}).unwrap();
    let size=Size{width:390,height:600};let constraints=Constraints::new(size,size).unwrap();
    let closed=host.render(app,window,constraints).unwrap();
    assert!(!closed.hit_targets.iter().any(|hit|hit.widget==id(4)));
    let header=closed.hit_targets.iter().find(|hit|hit.widget==id(2)).unwrap().rect;
    let point=Point{x:header.origin.x+15,y:header.origin.y+15};
    for kind in [PointerEventKind::Down,PointerEventKind::Up] { assert!(host.pointer_event(app,window,kind,point,1,0).unwrap().is_none()); }
    host.render(app,window,constraints).unwrap();
    assert!(host.windows[&(app,window)].navigation_maximum > 0);
    let start=Point{x:30,y:100};let end=Point{x:30,y:-2000};
    host.pointer_event(app,window,PointerEventKind::Down,start,1,0).unwrap();
    host.pointer_event(app,window,PointerEventKind::Move,end,1,0).unwrap();
    assert!(host.pointer_event(app,window,PointerEventKind::Up,end,1,0).unwrap().is_none());
    let scrolled=host.render(app,window,constraints).unwrap();
    assert!(scrolled.hit_targets.iter().any(|hit|hit.widget==id(39)), "swiping reaches the last location");
    // Closing and reopening starts at the first location without selecting on drag.
    for _ in 0..2 {for kind in [PointerEventKind::Down,PointerEventKind::Up] {host.pointer_event(app,window,kind,point,1,0).unwrap();}host.render(app,window,constraints).unwrap();}
    let opened=host.render(app,window,constraints).unwrap();
    let destination=opened.hit_targets.iter().find(|hit|hit.widget==id(4)).unwrap().rect;
    let point=Point{x:destination.origin.x+15,y:destination.origin.y+15};
    host.pointer_event(app,window,PointerEventKind::Down,point,1,0).unwrap();
    let event=host.pointer_event(app,window,PointerEventKind::Up,point,1,0).unwrap().unwrap();assert_eq!(event.target,id(4));
    let closed=host.render(app,window,constraints).unwrap();
    assert!(!closed.hit_targets.iter().any(|hit|hit.widget==id(4)));
    assert!(closed.hit_targets.iter().any(|hit|hit.widget==id(5)));
    host.compact_form_factor=false;
    let size=Size{width:1024,height:768};host.render(app,window,Constraints::new(size,size).unwrap()).unwrap();
    assert!(host.windows[&(app,window)].navigation_headers.is_empty());
    assert!(host.windows[&(app,window)].geometry.contains_key(&id(4)));
}

use super::*;
use rice_native_app_protocol::{UiPatch, UiPatchEnvelope, UiEvent, PatchValue};
use rice_ui::{WidgetKind, WidgetState};
fn id(n:u64)->WidgetId { WidgetId::new(n).unwrap() }
fn app()->AppId {AppId::new(812).unwrap()}
fn window()->WindowId {WindowId::new(1).unwrap()}
fn create(n:u64,parent:Option<u64>,kind:WidgetKind,state:WidgetState)->UiPatch {
    UiPatch::Create {id:id(n),parent:parent.map(id),index:0,kind:kind as u16,state:state.bits(),text:Some("value".into()),icon:None}
}
fn apply(t:&mut NativeUiTransport,sequence:u64,patches:Vec<UiPatch>) {
    t.apply(&Message::UiPatch(UiPatchEnvelope{app:app(),window:window(),sequence,patches}).encode().unwrap()).unwrap();
}
fn drain(t:&mut NativeUiTransport)->Vec<UiEvent> {
    let mut events=Vec::new(); while t.events.pending()>0 {
        let mut bytes=vec![0;t.events.pending()];t.events.drain(&mut bytes).unwrap();
        if let Message::UiEvent(e)=Message::decode(&bytes).unwrap(){events.push(e.event)}else{panic!("not UI")}
    } events
}
#[test]
fn resolved_sizes_coalesce_and_unchanged_layout_emits_nothing() {
    let mut t=NativeUiTransport::default();
    apply(&mut t,1,vec![create(1,None,WidgetKind::Column,WidgetState::ENABLED),UiPatch::ObserveSize{id:id(1),enabled:true}]);
    t.render(app(),window(),600,400).unwrap();t.render(app(),window(),800,500).unwrap();
    let sizes=t.host.observed_widget_sizes(app(),window());assert_eq!(sizes.len(),1);
    assert_eq!(drain(&mut t),vec![UiEvent::SizeChanged{width:sizes[0].1.width,height:sizes[0].1.height}]);
    t.render(app(),window(),800,500).unwrap();assert!(drain(&mut t).is_empty());
    apply(&mut t,2,vec![UiPatch::ObserveSize{id:id(1),enabled:false}]);
    t.render(app(),window(),400,300).unwrap();assert!(drain(&mut t).is_empty());
    apply(&mut t,3,vec![UiPatch::ObserveSize{id:id(1),enabled:true}]);
    t.render(app(),window(),400,300).unwrap();assert_eq!(drain(&mut t).len(),1);
}
#[test]
fn size_subscription_removal_and_root_replacement_drop_stale_events() {
    let mut t=NativeUiTransport::default();
    apply(&mut t,1,vec![create(1,None,WidgetKind::Column,WidgetState::ENABLED),create(2,Some(1),WidgetKind::Label,WidgetState::ENABLED),UiPatch::ObserveSize{id:id(2),enabled:true}]);
    t.render(app(),window(),600,400).unwrap();assert_eq!(t.events.frames.len(),1);
    apply(&mut t,2,vec![UiPatch::Remove{id:id(1)}]);assert!(drain(&mut t).is_empty());assert!(t.emitted_sizes.is_empty());
    apply(&mut t,3,vec![UiPatch::Clear,create(1,None,WidgetKind::Column,WidgetState::ENABLED),UiPatch::ObserveSize{id:id(1),enabled:true}]);
    t.render(app(),window(),600,400).unwrap();assert_eq!(drain(&mut t).len(),1);
}
#[test]
fn size_observer_limit_is_atomic_and_wire_roundtrips() {
    let mut t=NativeUiTransport::default();
    let mut patches=vec![];for n in 1..=65 {patches.push(UiPatch::Create{id:id(n),parent:None,index:(n-1)as u32,kind:WidgetKind::Label as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None});patches.push(UiPatch::ObserveSize{id:id(n),enabled:true});}
    let frame=Message::UiPatch(UiPatchEnvelope{app:app(),window:window(),sequence:1,patches});assert_eq!(Message::decode(&frame.encode().unwrap()).unwrap(),frame);
    assert!(t.apply(&frame.encode().unwrap()).is_err());assert!(t.host.observed_widget_sizes(app(),window()).is_empty());
    apply(&mut t,1,vec![create(1,None,WidgetKind::Label,WidgetState::ENABLED)]);
    t.render(app(),window(),600,400).unwrap();assert!(drain(&mut t).is_empty());
}
#[test]
fn textfield_opt_in_forwards_navigation_without_editing_or_focus_traversal() {
    let mut t=NativeUiTransport::default();
    apply(&mut t,1,vec![create(1,None,WidgetKind::TextField,WidgetState::ENABLED|WidgetState::FOCUSED|WidgetState::KEY_NAVIGATION)]);
    t.render(app(),window(),600,400).unwrap();
    for key in [23,110,111,113,114,115,116] {
        let e=t.host.key_event(app(),window(),true,key,0,None).unwrap().unwrap();
        assert!(matches!(e.event,UiEvent::Key{key_code,..} if key_code==key));assert_eq!(e.target,id(1));
    }
    let e=t.host.key_event(app(),window(),true,38,0,Some("a".into())).unwrap().unwrap();assert!(matches!(e.event,UiEvent::ValueChanged(_)),"printable keys retain text editing");
    apply(&mut t,2,vec![UiPatch::Update{id:id(1),state:PatchValue::Set((WidgetState::ENABLED|WidgetState::FOCUSED).bits()),text:PatchValue::Keep,icon:PatchValue::Keep}]);
    let e=t.host.key_event(app(),window(),true,111,0,None).unwrap().unwrap();assert!(matches!(e.event,UiEvent::Key{key_code:111,..}),"nonprinting keys must not fabricate text changes without navigation opt-in");
}
#[test]
fn virtual_grid_cursor_marks_only_fill_handle_and_actual_track_header_edges() {
    use crate::native_ui::PointerCursor;
    let mut t=NativeUiTransport::default();
    let frame=r#"{"version":1,"revision":1,"sheet":1,"row_count":10,"column_count":10,"width":240,"height":120,"row_header":40,"column_header":20,"rows":[{"index":0,"start":0,"extent":30,"clip_start":0,"clip_end":30,"frozen":false}],"columns":[{"index":0,"start":0,"extent":100,"clip_start":0,"clip_end":100,"frozen":false}],"strings":[],"styles":[{"alignment":0,"role":0}],"cells":[],"selection":{"first_row":0,"last_row":0,"first_column":0,"last_column":0},"active":[0,0],"fill_endpoint":[0,0]}"#;
    apply(&mut t,1,vec![UiPatch::Create{id:id(1),parent:None,index:0,kind:WidgetKind::VirtualGrid as u16,state:WidgetState::ENABLED.bits(),text:Some(frame.into()),icon:None}]);
    let constraints=Constraints::new(Size{width:240,height:120},Size{width:240,height:120}).unwrap();
    t.host.render(app(),window(),constraints).unwrap();
    for (x,y,cursor) in [(135,45,PointerCursor::Crosshair),(140,10,PointerCursor::ColumnResize),(20,50,PointerCursor::RowResize),(70,10,PointerCursor::Default),(80,35,PointerCursor::Default)] {
        t.host.pointer_event(app(),window(),PointerEventKind::Move,Point{x,y},0,0).unwrap();
        let output=t.host.render(app(),window(),constraints).unwrap();assert_eq!(output.cursor,cursor,"at {x},{y}");
    }
}
#[test]
fn thin_native_accessories_preserve_grid_area_and_fill_their_tracks() {
    let mut t=NativeUiTransport::default();
    let node=|n,parent:Option<u64>,index,kind:WidgetKind,text:&str|UiPatch::Create{id:id(n),parent:parent.map(id),index,kind:kind as u16,state:WidgetState::ENABLED.bits(),text:Some(text.into()),icon:None};
    apply(&mut t,1,vec![
        node(1,None,0,WidgetKind::Column,"panel:distribute"),
        node(2,Some(1),0,WidgetKind::Row,"panel:distribute"),
        node(3,Some(2),0,WidgetKind::Stack,"panel:flush"),
        node(4,Some(2),1,WidgetKind::Stack,"panel:flush:equal-fixed:accessory-width=16"),
        node(5,Some(4),0,WidgetKind::ScrollBar,"0/1000000"),
        node(6,Some(1),1,WidgetKind::Stack,"panel:flush:equal-fixed:accessory-height=16"),
        node(7,Some(6),0,WidgetKind::ScrollBar,"0/1000000"),
        UiPatch::ObserveSize{id:id(3),enabled:true},UiPatch::ObserveSize{id:id(5),enabled:true},UiPatch::ObserveSize{id:id(7),enabled:true},
    ]);
    for (width,height) in [(1280,900),(800,600),(320,240)] {
        t.render(app(),window(),width,height).unwrap();
        let sizes=t.host.observed_widget_sizes(app(),window()).into_iter().collect::<BTreeMap<_,_>>();
        let grid=sizes[&id(3)];let vertical=sizes[&id(5)];let horizontal=sizes[&id(7)];
        assert_eq!(vertical.width,16);assert_eq!(horizontal.height,16);
        assert_eq!(vertical.height,grid.height);assert_eq!(horizontal.width,width);
        assert!(grid.width>=width-24,"grid must retain all but the accessory and themed gap");
        assert!(grid.height>=height-24,"horizontal track must not consume half the viewport");
    }
}

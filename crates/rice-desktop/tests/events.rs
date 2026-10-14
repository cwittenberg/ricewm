use rice_desktop::{
    CoalescedEvent, DesktopEventCoalescer, EventPath, MonitorEvent, MountId, MountRecord,
};

fn path(value: &str) -> EventPath {
    EventPath::new(value)
}

fn mount(id: &str) -> MountRecord {
    MountRecord {
        id: MountId::new(id),
        label: String::from("Drive"),
        root: path("/media/drive"),
        read_only: false,
    }
}

#[test]
fn create_metadata_and_delete_collapse_without_stale_icon() {
    let mut events = DesktopEventCoalescer::new(8);
    events.push(MonitorEvent::Created(path("/desktop/a")));
    events.push(MonitorEvent::MetadataChanged(path("/desktop/a")));
    assert_eq!(
        events.drain(),
        vec![CoalescedEvent::Created(path("/desktop/a"))]
    );

    events.push(MonitorEvent::Created(path("/desktop/b")));
    events.push(MonitorEvent::Deleted(path("/desktop/b")));
    assert!(events.drain().is_empty());
}

#[test]
fn rename_chain_retains_original_source_and_final_target() {
    let mut events = DesktopEventCoalescer::new(8);
    events.push(MonitorEvent::Renamed {
        from: path("/desktop/a"),
        to: path("/desktop/b"),
    });
    events.push(MonitorEvent::Renamed {
        from: path("/desktop/b"),
        to: path("/desktop/c"),
    });
    assert_eq!(
        events.drain(),
        vec![CoalescedEvent::Renamed {
            from: path("/desktop/a"),
            to: path("/desktop/c")
        }]
    );
}

#[test]
fn delete_then_create_becomes_one_refresh() {
    let mut events = DesktopEventCoalescer::new(8);
    events.push(MonitorEvent::Deleted(path("/desktop/a")));
    events.push(MonitorEvent::Created(path("/desktop/a")));
    assert_eq!(
        events.drain(),
        vec![CoalescedEvent::MetadataChanged(path("/desktop/a"))]
    );
}

#[test]
fn transient_mount_arrival_and_removal_cancel() {
    let mut events = DesktopEventCoalescer::new(8);
    events.push(MonitorEvent::MountArrived(mount("usb")));
    events.push(MonitorEvent::MountRemoved(MountId::new("usb")));
    assert!(events.drain().is_empty());

    events.push(MonitorEvent::MountRemoved(MountId::new("usb")));
    events.push(MonitorEvent::MountArrived(mount("usb")));
    assert!(matches!(
        events.drain().as_slice(),
        [CoalescedEvent::MountArrived(record)] if record.id == MountId::new("usb")
    ));
}

#[test]
fn overflow_is_bounded_and_requests_atomic_rescan() {
    let mut events = DesktopEventCoalescer::new(2);
    for name in ["a", "b", "c", "d", "e"] {
        events.push(MonitorEvent::Created(path(name)));
    }
    assert!(events.is_overflowed());
    assert_eq!(events.len(), 0);
    assert!(matches!(
        events.drain().as_slice(),
        [CoalescedEvent::RescanRequired {
            dropped_at_least: 5
        }]
    ));
    assert!(!events.is_overflowed());
}

use rice_ui::IconId;

#[test]
fn computer_identity_roundtrips_without_replacing_desktop_or_home() {
    assert_eq!(IconId::Computer.wire_value(), 575);
    assert_eq!(IconId::from_wire(575), Some(IconId::Computer));
    assert_eq!(IconId::from_wire(70), Some(IconId::Desktop));
    assert_eq!(IconId::from_wire(5), Some(IconId::Home));
}

#[test]
fn drawing_tool_wire_ids_roundtrip_without_changing_sheet_icons() {
    let icons = [IconId::Shapes, IconId::Line, IconId::Rectangle, IconId::Ellipse,
        IconId::FillColor, IconId::OutlineColor, IconId::TextColor, IconId::Arrange,
        IconId::AlignObjects, IconId::DistributeHorizontal, IconId::DistributeVertical,
        IconId::Crop, IconId::TextBox, IconId::Rotate, IconId::BringForward, IconId::SendBackward];
    for (offset, icon) in icons.into_iter().enumerate() {
        let value = 182 + u16::try_from(offset).unwrap();
        assert_eq!(icon.wire_value(), value);
        assert_eq!(IconId::from_wire(value), Some(icon));
    }
    assert_eq!(IconId::from_wire(180), Some(IconId::DeleteRows));
    assert_eq!(IconId::from_wire(181), Some(IconId::DeleteColumns));
    assert_eq!(IconId::ShortcutApplication.wire_value(), 0x101);
}

#[test]
fn chart_family_icons_preserve_reserved_shortcut_wire_ids() {
    let icons = [
        (241,IconId::ChartColumn),
        (242,IconId::ChartLine),
        (243,IconId::ChartFormat),
        (244,IconId::ChartData),
        (245,IconId::ChartBar),
        (246,IconId::ChartArea),
        (247,IconId::ChartPie),
        (248,IconId::ChartDoughnut),
        (249,IconId::ChartScatter),
        (250,IconId::ChartBubble),
        (251,IconId::ChartRadar),
        (252,IconId::ChartStock),
        (253,IconId::ChartCombo),
        (254,IconId::ChartSurface),
        (255,IconId::ChartStatistical),
        (256,IconId::ChartWaterfall),
        (512,IconId::ChartFunnel),
        (513,IconId::ChartHierarchy),
    ];
    for (value,icon) in icons { assert_eq!(icon.wire_value(),value);assert_eq!(IconId::from_wire(value),Some(icon)); }
    assert_eq!(IconId::from_wire(0x101),Some(IconId::ShortcutApplication));
    assert_eq!(IconId::from_wire(0x102),Some(IconId::ShortcutDocument));
}

#[test]
fn zoom_icons_have_distinct_non_destructive_semantics_and_stable_wire_ids() {
    for (id, icon) in [(516, IconId::ZoomIn), (517, IconId::ZoomOut)] {
        assert_eq!(icon.wire_value(), id);
        assert_eq!(IconId::from_wire(id), Some(icon));
    }
    assert_eq!(IconId::from_wire(76), Some(IconId::ObjectAdd));
    assert_eq!(IconId::from_wire(77), Some(IconId::ObjectRemove));
    assert_eq!(IconId::from_wire(515), Some(IconId::FormatLineSpacing));
    assert_eq!(IconId::from_wire(0x101), Some(IconId::ShortcutApplication));
}

#[test]
fn spreadsheet_data_icons_are_distinct_and_roundtrip() {
    for (id,icon) in [(565,IconId::FormatWrapText),(566,IconId::FormatDecimalIncrease),(567,IconId::FormatDecimalDecrease),(568,IconId::DataRemoveDuplicates),(569,IconId::DataTextToColumns)] { assert_eq!(icon.wire_value(),id); assert_eq!(IconId::from_wire(id),Some(icon)); }
}

use super::*;

fn id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("test ID must be nonzero")
}

fn spec(value: u64, kind: WidgetKind) -> WidgetSpec {
    WidgetSpec {
        id: id(value),
        kind,
        state: WidgetState::ENABLED,
        text: None,
        icon: None,
    }
}

fn create(value: u64, kind: WidgetKind, parent: Option<u64>, index: usize) -> Patch {
    Patch::Create {
        widget: spec(value, kind),
        parent: parent.map(id),
        index,
    }
}

#[test]
fn geometry_and_default_theme_are_predictable() {
    let rect = Rect {
        origin: Point { x: 10, y: 20 },
        size: Size {
            width: 100,
            height: 50,
        },
    };
    assert!(rect.contains(Point { x: 10, y: 20 }));
    assert!(!rect.contains(Point { x: 110, y: 20 }));
    assert_eq!(
        rect.inset(Insets::all(4)),
        Rect {
            origin: Point { x: 14, y: 24 },
            size: Size {
                width: 92,
                height: 42,
            },
        }
    );

    let theme = Theme::ricewm_default();
    assert_eq!(theme.metrics.corner_radius, MODERN_CORNER_RADIUS);
    assert_eq!(theme.metrics.scrollbar_width, 16);
    assert_eq!(
        IconId::from_wire(IconId::DocumentSave.wire_value()),
        Some(IconId::DocumentSave)
    );
    assert_eq!(IconId::from_wire(u16::MAX), None);
    assert_eq!(IconId::from_wire(88), Some(IconId::Calendar));
    assert_eq!(IconId::from_wire(112), Some(IconId::NetworkMounts));
    assert_eq!(IconId::from_wire(134), Some(IconId::SubmenuRight));
    assert_eq!(IconId::from_wire(135), Some(IconId::SubmenuLeft));
    assert_eq!(IconId::from_wire(136), Some(IconId::WindowsLogo));
    assert_eq!(IconId::from_wire(141), Some(IconId::LinuxLogo));
    assert_eq!(IconId::from_wire(142), Some(IconId::SnapLogo));
    assert_eq!(IconId::from_wire(143), Some(IconId::Power));
    assert_eq!(IconId::from_wire(144), Some(IconId::Logout));
    assert_eq!(IconId::from_wire(145), Some(IconId::Reboot));
}

#[test]
fn catalog_theme_maps_and_validates_bounded_data() {
    let menu = [
        0x0001_0203,
        0x0011_1213,
        0x0021_2223,
        0x0031_3233,
        0x0041_4243,
        0x00f1_f2f3,
    ];
    let theme = CatalogTheme::new(
        [0x0011_1111; TASKBAR_COLOR_COUNT],
        menu,
        [0x0061_6263; FRAME_COLOR_COUNT],
        0,
    )
    .unwrap()
    .to_theme();
    assert_eq!(theme.palette.surface, Color::opaque(1, 2, 3));
    assert_eq!(
        theme.palette.surface_raised,
        Color::opaque(0x11, 0x12, 0x13)
    );
    assert_eq!(theme.palette.border, Color::opaque(0x21, 0x22, 0x23));
    assert_eq!(theme.palette.text_muted, Color::opaque(0x85, 0x86, 0x87));
    assert_eq!(theme.palette.focus, Color::opaque(0x61, 0x62, 0x63));
    assert_eq!(theme.metrics.corner_radius, 0);
    assert_eq!(
        CatalogTheme::new(
            [0x0100_0000; TASKBAR_COLOR_COUNT],
            [0; MENU_COLOR_COUNT],
            [0; FRAME_COLOR_COUNT],
            0,
        ),
        Err(CatalogThemeError::InvalidRgb)
    );
    assert_eq!(
        CatalogTheme::new(
            [0; TASKBAR_COLOR_COUNT],
            [0; MENU_COLOR_COUNT],
            [0; FRAME_COLOR_COUNT],
            MAX_CORNER_RADIUS + 1,
        ),
        Err(CatalogThemeError::CornerRadiusExceeded)
    );
}

#[test]
fn builds_updates_moves_and_removes_a_tree() {
    let mut tree = RetainedTree::default();
    tree.apply(&[
        create(1, WidgetKind::Column, None, 0),
        create(2, WidgetKind::Label, Some(1), 0),
        create(3, WidgetKind::Button, Some(1), 1),
    ])
    .expect("valid tree");
    assert_eq!(tree.roots(), &[id(1)]);
    assert_eq!(tree.get(id(1)).expect("root").children(), &[id(2), id(3)]);

    tree.apply(&[
        Patch::Update {
            id: id(3),
            state: Some(WidgetState::ENABLED | WidgetState::FOCUSED),
            text: ValueChange::Set("Save".into()),
            icon: ValueChange::Set(IconId::DocumentSave),
        },
        Patch::Move {
            id: id(3),
            parent: None,
            index: 1,
        },
    ])
    .expect("valid update and move");
    assert_eq!(tree.roots(), &[id(1), id(3)]);
    assert_eq!(
        tree.get(id(3)).expect("button").text.as_deref(),
        Some("Save")
    );

    tree.apply(&[Patch::Remove { id: id(1) }])
        .expect("remove subtree");
    assert_eq!(tree.len(), 1);
    assert!(tree.get(id(2)).is_none());
}

#[test]
fn batch_application_is_atomic() {
    let mut tree = RetainedTree::default();
    tree.apply(&[create(1, WidgetKind::Column, None, 0)])
        .expect("root");
    let original = tree.clone();
    let result = tree.apply(&[
        create(2, WidgetKind::Label, Some(1), 0),
        create(2, WidgetKind::Button, Some(1), 1),
    ]);
    assert_eq!(result, Err(TreeError::DuplicateWidget(id(2))));
    assert_eq!(tree, original);
}

#[test]
fn rejects_missing_parents_cycles_indexes_and_large_text() {
    let mut tree = RetainedTree::default();
    assert_eq!(
        tree.apply(&[create(1, WidgetKind::Label, Some(99), 0)]),
        Err(TreeError::MissingParent(id(99)))
    );
    assert_eq!(
        tree.apply(&[create(1, WidgetKind::Label, None, 1)]),
        Err(TreeError::InvalidIndex {
            index: 1,
            length: 0
        })
    );

    tree.apply(&[
        create(1, WidgetKind::Column, None, 0),
        create(2, WidgetKind::Column, Some(1), 0),
    ])
    .expect("nested tree");
    assert_eq!(
        tree.apply(&[Patch::Move {
            id: id(1),
            parent: Some(id(2)),
            index: 0,
        }]),
        Err(TreeError::Cycle)
    );

    assert!(matches!(
        tree.apply(&[Patch::Update {
            id: id(2),
            state: None,
            text: ValueChange::Set("x".repeat(MAX_WIDGET_TEXT_BYTES + 1)),
            icon: ValueChange::Keep,
        }]),
        Err(TreeError::TextTooLong { .. })
    ));
}

#[test]
fn rejects_oversized_batches() {
    let mut tree = RetainedTree::default();
    let patches = vec![Patch::Clear; MAX_PATCH_BATCH + 1];
    assert_eq!(
        tree.apply(&patches),
        Err(TreeError::LimitExceeded {
            resource: "patch batch",
            maximum: MAX_PATCH_BATCH,
        })
    );
}

#[test]
fn protocol_conversion_preserves_patch_semantics() {
    let patch = Patch::Update {
        id: id(7),
        state: Some(WidgetState::ENABLED | WidgetState::SELECTED),
        text: ValueChange::Clear,
        icon: ValueChange::Set(IconId::FolderOpen),
    };
    let wire = patch.to_protocol().expect("patch is representable");
    assert_eq!(Patch::try_from(&wire), Ok(patch));

    let unknown = rice_native_app_protocol::UiPatch::Create {
        id: id(8),
        parent: None,
        index: 0,
        kind: u16::MAX,
        state: 0,
        text: None,
        icon: None,
    };
    assert_eq!(
        Patch::try_from(&unknown),
        Err(PatchConversionError::UnknownWidgetKind(u16::MAX))
    );
}

#[test]
fn file_icons_cover_named_formats_and_mime_fallbacks() {
    for (name, mime, expected) in [
        ("song.flac", None, IconId::Audio),
        (
            "module.wasm",
            Some("application/octet-stream"),
            IconId::Binary,
        ),
        ("layout.css", Some("text/plain"), IconId::Css),
        ("table.csv", None, IconId::Csv),
        ("report.docx", None, IconId::WordDocument),
        ("Dockerfile", None, IconId::Docker),
        (".env.production", None, IconId::Env),
        ("bundle.zip", None, IconId::Archive),
        ("index.html", None, IconId::Html),
        ("app.mjs", None, IconId::JavaScript),
        ("metadata.json", None, IconId::Manifest),
        ("site.webmanifest", None, IconId::Manifest),
        ("README.md", None, IconId::Markdown),
        ("paper.pdf", None, IconId::Pdf),
        ("slides.pptx", None, IconId::PresentationFile),
        ("launch.sh", None, IconId::Program),
        ("private.pem", None, IconId::Secret),
        ("notes.txt", None, IconId::Text),
        ("LICENSE", None, IconId::Text),
        (".gitignore", None, IconId::Text),
        ("movie.webm", None, IconId::Video),
        ("budget.xlsx", None, IconId::SpreadsheetFile),
        ("BUDGET.XLS", Some("application/octet-stream"), IconId::SpreadsheetFile),
        ("slides.PPTX", Some("application/zip"), IconId::PresentationFile),
        ("template.POTX", None, IconId::PresentationFile),
        ("unnamed", Some("application/vnd.ms-excel"), IconId::SpreadsheetFile),
        ("unnamed", Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"), IconId::PresentationFile),
        ("compose.yaml", None, IconId::Yaml),
        ("photo.png", None, IconId::Image),
    ] {
        assert_eq!(file_icon(name, mime), expected, "{name}");
    }
}

#[test]
fn special_folder_icons_follow_the_folder_identity() {
    assert_eq!(folder_icon("/root/Desktop"), IconId::Desktop);
    assert_eq!(folder_icon("/root/Documents/"), IconId::Documents);
    assert_eq!(folder_icon("/root/Downloads"), IconId::Downloads);
    assert_eq!(folder_icon("/root/Pictures"), IconId::Pictures);
    assert_eq!(folder_icon("/mnt/local"), IconId::NetworkMounts);
    assert_eq!(
        folder_icon("/mnt/local/Work/Projects"),
        IconId::NetworkMounts
    );
    assert_eq!(
        folder_icon("/run/ricewm/mounts/team/Documents"),
        IconId::NetworkMounts
    );
    assert_eq!(folder_icon("/root"), IconId::Home);
    assert_eq!(folder_icon("/root/Projects"), IconId::Folder);
}

#[test]
fn inline_images_have_a_bounded_image_allowance_without_expanding_labels() {
    let mut tree = RetainedTree::default();
    tree.apply(&[create(1, WidgetKind::Image, None, 0), create(2, WidgetKind::Label, None, 1)]).unwrap();
    let payload = format!("inline-image:data:image/webp;base64,{}", "A".repeat(48 * 1024));
    let update = |widget, text| Patch::Update { id: id(widget), state: None, text: ValueChange::Set(text), icon: ValueChange::Keep };
    tree.apply(&[update(1, payload.clone())]).unwrap();
    assert!(matches!(tree.apply(&[update(2, payload)]), Err(TreeError::TextTooLong { .. })));
    assert!(matches!(tree.apply(&[update(1, format!("inline-image:data:image/webp;base64,{}", "A".repeat(MAX_INLINE_IMAGE_TEXT_BYTES)))]), Err(TreeError::TextTooLong { .. })));
}

#[test]
fn spreadsheet_format_icons_have_stable_native_wire_ids() {
    for (id,icon) in [(236,IconId::CellStyles),(237,IconId::CellBorders),(238,IconId::FormatCells)] {
        assert_eq!(IconId::from_wire(id),Some(icon));assert_eq!(icon.wire_value(),id);
    }
}

#[test]
fn document_alignment_metadata_is_optional_strict_and_atomic() {
    let mut tree=RetainedTree::default();
    tree.apply(&[create(1,WidgetKind::TextField,None,0)]).unwrap();
    for vertical in [None,Some("top"),Some("center"),Some("bottom")] {
        let mut value=serde_json::json!({"family":"Arial","size_pt":11.25,"weight":400,"italic":false,"underline":false,"paint":{"background":"#FFFFFF"}});
        if let Some(vertical)=vertical {value["paint"]["vertical"]=vertical.into();}
        tree.apply(&[Patch::DocumentFont{id:id(1),font:Some(value.to_string())}]).unwrap();
        assert_eq!(tree.document_paint(id(1)).unwrap().vertical.as_deref(),vertical);
        assert_eq!(tree.document_font(id(1)).unwrap().size_pt.points(),11.25);
    }
    let before=tree.document_paint(id(1)).unwrap().clone();
    for invalid in [serde_json::Value::Null,serde_json::json!(1),serde_json::json!(false),serde_json::json!([]),serde_json::json!({}),serde_json::json!("middle"),serde_json::json!("Bottom"),serde_json::json!("bottom\n")] {
        let font=serde_json::json!({"family":"Arial","size_pt":24,"weight":400,"italic":false,"underline":false,"paint":{"background":"#000000","vertical":invalid}}).to_string();
        assert_eq!(tree.apply(&[Patch::DocumentFont{id:id(1),font:Some(font)}]),Err(TreeError::InvalidDocumentFont));
        assert_eq!(tree.document_paint(id(1)),Some(&before));
        assert_eq!(tree.document_font(id(1)).unwrap().size_pt.points(),11.25);
    }
    let legacy:crate::grid_frame::DocumentPaint=serde_json::from_str(r##"{"background":"#FFFFFF","foreground":"#123456"}"##).unwrap();
    assert!(legacy.is_valid());assert_eq!(legacy.vertical,None);
    assert_eq!(serde_json::to_string(&legacy).unwrap(),r##"{"background":"#FFFFFF","foreground":"#123456"}"##);
    tree.apply(&[Patch::DocumentFont{id:id(1),font:None}]).unwrap();
    assert!(tree.document_font(id(1)).is_none());assert!(tree.document_paint(id(1)).is_none());
}

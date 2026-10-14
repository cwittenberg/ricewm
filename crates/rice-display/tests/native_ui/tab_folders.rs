use super::*;
use rice_ui::Insets;

fn catalog_themes() -> Vec<(String, Theme)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes");
    let mut themes = Vec::new();
    for file in std::fs::read_dir(path).unwrap() {
        let path = file.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("theme") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // The catalog contains both legacy key/value presets and v2 bundles.
        // Decode every theme rather than silently excluding the JSON entries.
        let fields: BTreeMap<String, String> = if text.trim_start().starts_with('{') {
            let bundle: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(bundle["$schema"], "ricewm.theme/v2", "{}", path.display());
            let mut fields = bundle["appearance"]
                .as_object()
                .expect("v2 theme appearance")
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        value.as_str().expect("theme string value").to_owned(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            fields.insert(
                "name".to_owned(),
                bundle["name"].as_str().expect("theme name").to_owned(),
            );
            fields
        } else {
            text.lines()
                .filter_map(|line| line.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        };
        let rgb = |key| u32::from_str_radix(&fields[key], 16).unwrap();
        let mut record = b"RTUI\x01\x00\x60\x00".to_vec();
        let mut radius = fields
            .get("corner.radius")
            .map_or("6", String::as_str)
            .parse::<u16>()
            .unwrap();
        let mut flags = match fields.get("chrome.profile").map(String::as_str) {
            Some("familiar") => 1 << 5,
            Some("yellow-jacket") => 1 << 6,
            Some("command-deck") => 1 << 7,
            Some("orchard") => 1 << 8,
            Some("neon-grid") => 1 << 9,
            Some("redmond-98") => {
                radius |= 1 << 15;
                0
            }
            _ => 0,
        };
        if fields.get("material").map(String::as_str) == Some("liquid-glass") {
            flags |= 1;
        }
        // These legacy presets get their catalog defaults from their stable filenames.
        match path.file_name().and_then(|name| name.to_str()) {
            Some("classic.theme") => {
                radius = 0;
                flags |= 1 << 5;
            }
            Some("emerald.theme" | "frosted-ice.theme") => {
                flags |= 1;
            }
            _ => {}
        }
        record.extend_from_slice(&radius.to_le_bytes());
        record.extend_from_slice(&(flags as u16).to_le_bytes());
        for key in [
            "taskbar.bar",
            "taskbar.ridge",
            "taskbar.start",
            "taskbar.task",
            "taskbar.tray",
            "taskbar.text",
            "accent",
            "menu.background",
            "menu.alternate",
            "menu.separator",
            "menu.hover",
            "menu.selected",
            "menu.text",
            "frame.active",
            "frame.inactive",
            "frame.border",
            "frame.body",
            "frame.text",
            "frame.control",
            "frame.close",
            "frame.pressed",
        ] {
            record.extend_from_slice(&rgb(key).to_le_bytes());
        }
        themes.push((
            fields["name"].to_string(),
            crate::native_ui_theme::decode_theme(&record).unwrap(),
        ));
    }
    themes.sort_by(|a, b| a.0.cmp(&b.0));
    themes
}

// Sample only straight-edge chrome, away from text and rounded corners.
fn chrome_pixel(commands: &[DrawCommand], point: Point) -> Option<Color> {
    let mut pixel = None;
    let mut clips = Vec::new();
    for command in commands {
        match command {
            DrawCommand::PushClip { rect } => {
                clips.push(*rect);
                continue;
            }
            DrawCommand::PopClip => {
                clips.pop();
                continue;
            }
            _ => {}
        }
        if clips.iter().any(|rect| !rect.contains(point)) {
            continue;
        }
        match command {
            DrawCommand::Fill { rect, color, .. } if rect.contains(point) => pixel = Some(*color),
            DrawCommand::Border {
                rect, color, width, ..
            } if rect.contains(point)
                && !rect.inset(Insets::all(u32::from(*width))).contains(point) =>
            {
                pixel = Some(*color)
            }
            _ => {}
        }
    }
    pixel
}

#[test]
fn every_theme_opens_the_selected_tab_into_its_page_on_all_three_edges() {
    for (name, theme) in catalog_themes() {
        for axis in ["horizontal", "vertical", "vertical:right"] {
            for selected in [2, 3] {
                let mut host = NativeUiHost::new(theme);
                host.apply_patch(&envelope(
                    1,
                    vec![
                        create(
                            1,
                            None,
                            0,
                            WidgetKind::Tabs,
                            WidgetState::ENABLED,
                            Some(&format!("tabs:{axis}:integrated")),
                            None,
                        ),
                        create(
                            2,
                            Some(1),
                            0,
                            WidgetKind::Row,
                            WidgetState::ENABLED
                                | if selected == 2 {
                                    WidgetState::SELECTED
                                } else {
                                    WidgetState::empty()
                                },
                            Some("panel:tab"),
                            None,
                        ),
                        create(
                            3,
                            Some(1),
                            1,
                            WidgetKind::Row,
                            WidgetState::ENABLED
                                | if selected == 3 {
                                    WidgetState::SELECTED
                                } else {
                                    WidgetState::empty()
                                },
                            Some("panel:tab"),
                            None,
                        ),
                        create(
                            4,
                            Some(2),
                            0,
                            WidgetKind::Label,
                            WidgetState::ENABLED,
                            Some("First tab"),
                            None,
                        ),
                        create(
                            5,
                            Some(3),
                            0,
                            WidgetKind::Label,
                            WidgetState::ENABLED,
                            Some("Second tab"),
                            None,
                        ),
                        create(
                            6,
                            Some(1),
                            2,
                            WidgetKind::Container,
                            WidgetState::ENABLED | WidgetState::EXPANSIVE,
                            Some("panel:tab-page:flex"),
                            None,
                        ),
                    ],
                ))
                .unwrap();
                let output = host
                    .render(app(1), window(1), fixed_constraints(360, 180))
                    .unwrap();
                let geometry = &host.windows[&(app(1), window(1))].geometry;
                let active = geometry[&widget(selected)];
                let inactive = geometry[&widget(if selected == 2 { 3 } else { 2 })];
                let page = geometry[&widget(6)];
                let seam_point = |tab: Rect| match axis {
                    "vertical" => Point {
                        x: page.origin.x,
                        y: tab.origin.y + tab.size.height as i32 / 2,
                    },
                    "vertical:right" => Point {
                        x: page.origin.x + page.size.width as i32 - 1,
                        y: tab.origin.y + tab.size.height as i32 / 2,
                    },
                    _ => Point {
                        x: tab.origin.x + tab.size.width as i32 / 2,
                        y: page.origin.y,
                    },
                };
                let face = theme.palette.surface.with_alpha(255);
                assert_eq!(
                    chrome_pixel(&output.commands, seam_point(active)),
                    Some(face),
                    "{name} {axis}: active tab must erase the page seam"
                );
                assert_ne!(
                    chrome_pixel(&output.commands, seam_point(inactive)),
                    Some(face),
                    "{name} {axis}: inactive tab must retain a separating edge"
                );
            }
        }
    }
}

#[test]
fn plain_tab_buttons_join_the_following_app_panel() {
    for (name, theme) in catalog_themes() {
        let mut host = NativeUiHost::new(theme);
        host.apply_patch(&envelope(
            1,
            vec![
                create(
                    1,
                    None,
                    0,
                    WidgetKind::Column,
                    WidgetState::ENABLED,
                    None,
                    None,
                ),
                create(
                    2,
                    Some(1),
                    0,
                    WidgetKind::Tabs,
                    WidgetState::ENABLED,
                    None,
                    None,
                ),
                create(
                    3,
                    Some(2),
                    0,
                    WidgetKind::Button,
                    WidgetState::ENABLED | WidgetState::SELECTED,
                    Some("Overview"),
                    None,
                ),
                create(
                    4,
                    Some(2),
                    1,
                    WidgetKind::Button,
                    WidgetState::ENABLED,
                    Some("Details"),
                    None,
                ),
                create(
                    5,
                    Some(1),
                    1,
                    WidgetKind::Column,
                    WidgetState::ENABLED | WidgetState::EXPANSIVE,
                    None,
                    None,
                ),
            ],
        ))
        .unwrap();
        let output = host
            .render(app(1), window(1), fixed_constraints(360, 180))
            .unwrap();
        let geometry = &host.windows[&(app(1), window(1))].geometry;
        let active = geometry[&widget(3)];
        let page = geometry[&widget(5)];
        assert_eq!(
            active.origin.y + active.size.height as i32,
            page.origin.y,
            "{name}: no gutter between tabs and page"
        );
        let point = Point {
            x: active.origin.x + active.size.width as i32 / 2,
            y: page.origin.y,
        };
        assert_eq!(
            chrome_pixel(&output.commands, point),
            Some(theme.palette.surface.with_alpha(255)),
            "{name}: panel border must open under active tab"
        );
        assert!(
            output
                .hit_targets
                .iter()
                .any(|target| target.widget == widget(4) && target.activatable)
        );
    }
}

#[test]
fn inactive_tab_labels_remain_legible_without_nested_borders_when_hovered_or_focused() {
    for (name, theme) in catalog_themes()
        .into_iter()
        .filter(|(name, _)| name == "Command Deck" || name == "Yellow Jacket")
    {
        let mut host = NativeUiHost::new(theme);
        host.apply_patch(&envelope(
            1,
            vec![
                create(
                    1,
                    None,
                    0,
                    WidgetKind::Tabs,
                    WidgetState::ENABLED,
                    Some("tabs:horizontal:integrated"),
                    None,
                ),
                create(
                    2,
                    Some(1),
                    0,
                    WidgetKind::Row,
                    WidgetState::ENABLED,
                    Some("panel:tab"),
                    None,
                ),
                create(
                    3,
                    Some(2),
                    0,
                    WidgetKind::Button,
                    WidgetState::ENABLED | WidgetState::FLAT | WidgetState::FOCUSED,
                    Some("Inactive folder"),
                    None,
                ),
            ],
        ))
        .unwrap();
        host.render(app(1), window(1), fixed_constraints(360, 40))
            .unwrap();
        let label = host.windows[&(app(1), window(1))].geometry[&widget(3)];
        host.pointer_event(
            app(1),
            window(1),
            PointerEventKind::Move,
            Point {
                x: label.origin.x + 10,
                y: label.origin.y + 10,
            },
            0,
            0,
        )
        .unwrap();
        let output = host
            .render(app(1), window(1), fixed_constraints(360, 40))
            .unwrap();
        assert!(output.commands.iter().any(|command| matches!(command,
        DrawCommand::Text { text, color, .. } if text == "Inactive folder" && *color == if name == "Command Deck" { Color::opaque(255,255,255) } else { theme.palette.text }
    )));
        let label = host.windows[&(app(1), window(1))].geometry[&widget(3)];
        assert!(
            !output.commands.iter().any(|command| matches!(command,
                DrawCommand::Border { rect, .. } if rect.origin.x > label.origin.x
                    && rect.origin.y > label.origin.y
                    && rect.size.width < label.size.width
                    && rect.size.height < label.size.height
            )),
            "tab labels must not paint their own inset button/focus borders"
        );
    }
}

#[test]
fn familiar_ribbon_and_sheet_tabs_use_surface_ink_instead_of_system_highlight_ink() {
    for (theme, expected) in [(Theme::familiar_light(), Color::opaque(10, 8, 13)), (Theme::familiar_dark(), Color::opaque(255, 255, 255))] {
        let mut host = NativeUiHost::new(theme.clone());
        let mut patches = vec![create(1, None, 0, WidgetKind::Tabs, WidgetState::ENABLED,
            Some("panel:tabs:integrated:flush:fixed:tight-stack"), None)];
        for (index, (name, state)) in [
            ("Home", WidgetState::ENABLED | WidgetState::CHECKED),
            ("Insert", WidgetState::ENABLED),
            ("Unavailable", WidgetState::default()),
        ].into_iter().enumerate() {
            patches.push(create(index as u64 + 2, Some(1), index as u32, WidgetKind::Button, state, Some(name), Some(IconId::Document)));
        }
        host.apply_patch(&envelope(1, patches)).unwrap();
        let output = host.render(app(1), window(1), fixed_constraints(500, 50)).unwrap();
        for label in ["Home", "Insert", "Unavailable"] {
            let actual = output.commands.iter().find_map(|command| match command {
                DrawCommand::Text { text, color, .. } if text == label => Some(*color), _ => None,
            }).expect("native tab text");
            assert_eq!(actual, expected.with_alpha(if label == "Unavailable" {128} else {255}), "{label}");
        }
    }
}

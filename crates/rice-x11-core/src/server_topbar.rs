//! Shared geometry for retained extension icons and canvas-owned shell actions.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct TopbarItem {
    pub key: String,
    pub title: String,
    pub icon: String,
    pub x: i16,
    pub width: u16,
    pub position: u8,
}
impl DisplayServer {
    pub fn configure_topbar(&mut self, source: &str) -> bool {
        if source.len() > 16384 || source.contains('\0') {
            return false;
        }
        if self.wm_topbar_config.as_deref() == Some(source) {
            return true;
        }
        self.wm_topbar_config = Some(source.to_owned());
        self.repaint_wm_chrome();
        true
    }
    pub fn topbar_metadata(&mut self, source: &str) -> bool {
        if source.len() > 16384 || source.contains('\0') {
            return false;
        }
        if self.wm_topbar_metadata == source {
            return true;
        }
        self.wm_topbar_metadata = source.to_owned();
        self.repaint_wm_chrome();
        true
    }
    pub(super) fn wm_topbar_layout(&self) -> Vec<TopbarItem> {
        if self.wm_topbar_config.is_none() || !self.wm_gnome_panel {
            return Vec::new();
        }
        let metadata: Vec<Vec<&str>> = self
            .wm_topbar_metadata
            .lines()
            .map(|s| s.split('\t').collect())
            .collect();
        let prefs: Vec<Vec<&str>> = self
            .wm_topbar_config
            .as_deref()
            .unwrap_or("")
            .lines()
            .map(|s| s.split('\t').collect())
            .collect();
        let mut items = Vec::new();
        let mut extensions: Vec<_> = self.wm_gnome_indicators.iter().enumerate().collect();
        extensions.sort_by_key(|(i, a)| (a.position, a.index, *i));
        // Status-area insertion order grows inward in GNOME. Convert once to
        // left-to-right order; every subsequent layout uses this same order.
        let mut right: Vec<_> = extensions
            .iter()
            .filter(|(_, a)| a.position == 2)
            .copied()
            .collect();
        right.reverse();
        let ordered = extensions
            .iter()
            .filter(|(_, a)| a.position != 2)
            .copied()
            .chain(right);
        for (_, a) in ordered {
            if !a.visible {
                continue;
            }
            let key = String::from_utf8_lossy(&a.uuid).into_owned();
            let meta = metadata
                .iter()
                .find(|r| r.first().copied() == Some(key.as_str()));
            let title = meta
                .and_then(|r| r.get(1))
                .copied()
                .unwrap_or(&key)
                .to_owned();
            let icon = meta
                .and_then(|r| r.get(2))
                .copied()
                .unwrap_or("")
                .to_owned();
            let width = if a.label.is_empty() {
                32
            } else {
                // Resource strips need more than the former 24-character label.
                // Bound each indicator to 45% of the output so controls retain room.
                ((String::from_utf8_lossy(&a.label).chars().take(96).map(|c| if c.is_ascii() { 8 } else { 16 }).sum::<usize>()
                    + usize::from(a.has_icon) * 20 + 16)
                    .min((usize::from(self.config.width_px) * 45 / 100).max(120))) as u16
            };
            items.push(TopbarItem {
                key,
                title,
                icon,
                x: 0,
                width,
                position: a.position,
            });
        }
        items.push(TopbarItem {
            key: "@clock".into(),
            title: rice_i18n::t("shell.topbar.calendar").into(),
            icon: "calendar".into(),
            x: 0,
            width: 84,
            position: 1,
        });
        let muted = self.wm_system_controls & 32 != 0;
        for (key, title, icon, width) in [
            ("@applications", "Rice Hub", "view-grid", 32),
            ("@search", rice_i18n::t("shell.topbar.search"), "search", 32),
            ("@screenshot", rice_i18n::t("shell.topbar.screenshot"), "camera", 32),
            ("@mail", rice_i18n::t("shell.topbar.mail"), "mail-read", 48),
            ("@network", rice_i18n::t("shell.topbar.network"), "network", 32),
            (
                "@audio",
                rice_i18n::t("shell.topbar.sound"),
                if muted { "audio-muted" } else { "audio-volume" },
                32,
            ),
            ("@battery", rice_i18n::t("shell.topbar.battery"), "battery", 52),
            (
                "@session",
                rice_i18n::t("shell.topbar.session"),
                "logout",
                32,
            ),
        ] {
            if matches!(key, "@network" | "@audio" | "@battery" | "@session")
                && self.wm_system_controls & 0x41 != 1
            {
                continue;
            }
            if key == "@mail" && self.wm_mail_unread.is_none() {
                continue;
            }
            if key == "@battery" && self.wm_battery_percentage.is_none() {
                continue;
            }
            items.push(TopbarItem {
                key: key.into(),
                title: title.into(),
                icon: icon.into(),
                x: 0,
                width,
                position: 2,
            });
        }
        for item in &mut items {
            if let Some(row) = prefs
                .iter()
                .find(|r| r.first().copied() == Some(item.key.as_str()))
            {
                if row.get(1) == Some(&"0") {
                    item.width = 0;
                }
                if let Some(position) = row
                    .get(2)
                    .and_then(|s| s.parse::<u8>().ok())
                    .filter(|p| *p <= 2)
                {
                    item.position = position;
                }
                if let Some(icon) = row.get(3).filter(|s| !s.is_empty()) {
                    item.icon = (*icon).to_owned();
                }
            }
        }
        items.sort_by_key(|item| {
            (
                item.position,
                prefs
                    .iter()
                    .position(|r| r.first().copied() == Some(item.key.as_str()))
                    .unwrap_or(usize::MAX),
                match item.key.as_str() {
                    "@applications" => 0,
                    "@search" => 1,
                    "@screenshot" => 2,
                    _ => 3,
                },
            )
        });
        let panel_width = i32::from(self.wm_panel_output_rect().2);
        let mut left = 8;
        let mut right = panel_width - 8;
        for item in items.iter_mut().filter(|i| i.position == 0 && i.width > 0) {
            item.x = left as i16;
            left += i32::from(item.width) + 4;
        }
        for item in items
            .iter_mut()
            .rev()
            .filter(|i| i.position == 2 && i.width > 0)
        {
            let x = right - i32::from(item.width);
            if x < left {
                item.width = 0;
                continue;
            }
            item.x = x as i16;
            right = x - 4;
        }
        let center_width: i32 = items
            .iter()
            .filter(|i| i.position == 1 && i.width > 0)
            .map(|i| i32::from(i.width) + 4)
            .sum();
        let mut center = ((panel_width - center_width) / 2)
            .min(right - center_width)
            .max(left);
        for item in items.iter_mut().filter(|i| i.position == 1 && i.width > 0) {
            if center + i32::from(item.width) > right {
                item.width = 0;
                continue;
            }
            item.x = center as i16;
            center += i32::from(item.width) + 4;
        }
        items
    }
}

/// Retained application notification icon. Slots are shell-owned, never HWNDs.
#[derive(Clone, Debug)]
struct WmNotificationIcon {
    slot: u32,
    tip: Vec<u8>,
    width: u16,
    height: u16,
    pixels: Vec<u32>,
    hidden: bool,
}
impl DisplayServer {
    fn notification_icon_layout(&self) -> Vec<(u32, usize, usize)> {
        if self.wm_compact_shell() { return Vec::new(); }
        let (_, _, width, height) = self.wm_taskbar_rect();
        let vertical = self.wm_taskbar_edge.vertical();
        let visible = self.wm_notification_icons.iter().filter(|icon| !icon.hidden).collect::<Vec<_>>();
        let end = if self.wm_dock_style {
            if vertical { usize::from(height) } else { usize::from(width) }.saturating_sub(WM_COMPACT_DOCK_AXIS_PADDING)
        } else if vertical {
            self.wm_vertical_tray_layout().first().map_or(usize::from(height), |entry| entry.2)
        } else {
            self.wm_tray_layout().first().map_or(usize::from(width), |entry| entry.2)
        };
        let start = end.saturating_sub(visible.len()*28);
        visible.into_iter().enumerate().map(|(index, icon)| {
            let axis = start + index*28;
            if vertical { (icon.slot, usize::from(width).saturating_sub(24)/2, axis) }
            else { (icon.slot, axis, usize::from(height).saturating_sub(24)/2) }
        }).collect()
    }
    fn notification_icon_at(&self, root_x:i16, root_y:i16) -> Option<u32> {
        let (x,y,_,_) = self.wm_taskbar_rect();
        let lx = i32::from(root_x)-i32::from(x); let ly=i32::from(root_y)-i32::from(y);
        self.notification_icon_layout().into_iter().find_map(|(slot,x,y)|
            ((x as i32..x as i32+24).contains(&lx) && (y as i32..y as i32+24).contains(&ly)).then_some(slot))
    }
    pub fn update_notification_icon(&mut self, slot:u32, tip:Vec<u8>, width:u16, height:u16, pixels:Vec<u32>, hidden:bool) -> bool {
        if !self.config.native_window_manager || slot == 0 || tip.len()>512 || std::str::from_utf8(&tip).is_err() || width==0 || height==0 || width>128 || height>128 || pixels.len()!=usize::from(width)*usize::from(height) { return false; }
        let icon = WmNotificationIcon {slot,tip,width,height,pixels,hidden};
        if let Some(existing)=self.wm_notification_icons.iter_mut().find(|icon| icon.slot==slot) { *existing=icon; }
        else { if self.wm_notification_icons.len()>=64 { return false; } self.wm_notification_icons.push(icon); }
        self.refresh_taskbar_geometry(); self.paint_wm_chrome(WM_TASKBAR); self.scene_upsert(WM_TASKBAR); true
    }
    pub fn remove_notification_icon(&mut self, slot:u32) -> bool {
        let Some(index)=self.wm_notification_icons.iter().position(|icon| icon.slot==slot) else {return false;};
        self.wm_notification_icons.remove(index);
        self.wm_notification_events.retain(|event| event[0]!=slot);
        if self.wm_notification_focus==Some(slot) {self.wm_notification_focus=None;}
        if self.wm_chrome_hover==Some(WmChromeTarget::NotificationIcon(slot)) {self.wm_chrome_hover=None;}
        self.refresh_taskbar_geometry(); self.update_wm_quick_launch_tooltip(); self.paint_wm_chrome(WM_TASKBAR); self.scene_upsert(WM_TASKBAR); true
    }
    pub fn focus_notification_icon(&mut self, slot:u32) -> bool {
        if !self.wm_notification_icons.iter().any(|icon| icon.slot==slot && !icon.hidden) {return false;}
        self.wm_notification_focus=Some(slot); self.paint_wm_chrome(WM_TASKBAR); self.scene_upsert(WM_TASKBAR); true
    }
    fn push_notification_event(&mut self, slot:u32,event:u32,x:i16,y:i16) {
        if self.wm_notification_events.len()<1024 {self.wm_notification_events.push_back([slot,event,x as i32 as u32,y as i32 as u32]);}
    }
    pub fn drain_notification_event(&mut self) -> Option<[u32;4]> {self.wm_notification_events.pop_front()}
}

#[cfg(test)]
#[path = "../../../tests/notification-area-compositor.rs"]
mod notification_area_tests;

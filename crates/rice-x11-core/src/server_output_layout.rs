use super::*;

fn valid_output_layout(width:u16,height:u16,outputs:&[(u16,u16,u16,u16)]) -> bool {
    width>0 && height>0 && outputs.len()<=16 && outputs.iter().all(|&(x,y,w,h)|
        w>0 && h>0 && x<=i16::MAX as u16 && y<=i16::MAX as u16
        && u32::from(x)+u32::from(w)<=u32::from(width)
        && u32::from(y)+u32::from(h)<=u32::from(height))
}


#[derive(Clone, Copy, Debug)]
pub(super) struct X11OutputHome {
    slot:u32,
    output:(u16,u16,u16,u16),
    geometry:(i16,i16,u16,u16),
    automatic:Option<(i16,i16,u16,u16)>,
    fullscreen:Option<(i16,i16,u16,u16)>,
    maximized:Option<(i16,i16,u16,u16)>,
    snapped:Option<WmSnapState>,
}

// Passive presentations of the shared panel/dock. Pointer entry moves the
// canonical interactive surface onto that output before hit testing.
const SHELL_REPLICA_BASE: u32 = 0xe000_0100;
pub(super) fn shell_replica_source(id: ResourceId) -> Option<ResourceId> {
    (SHELL_REPLICA_BASE..SHELL_REPLICA_BASE + 32).contains(&id.0)
        .then(|| if (id.0 - SHELL_REPLICA_BASE) % 2 == 0 { WM_GNOME_PANEL } else { WM_TASKBAR })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_monitor_panels_keep_geometry_hit_targets_and_workareas_together() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1800, height_px: 1000, native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.wm_gnome_panel = true;
        server.configure_topbar("");
        let outputs = vec![(0,0,800,600),(800,200,1000,800)];
        assert!(server.set_output_layout(1800,1000,outputs.clone()));
        let panel2=ResourceId(SHELL_REPLICA_BASE+2);
        let dock2=ResourceId(SHELL_REPLICA_BASE+3);
        let panel=&server.windows[&panel2.0];
        assert_eq!((panel.x,panel.y,panel.width,panel.height),(800,200,1000,32));
        assert!(panel.mapped);
        assert!(!server.windows.contains_key(&dock2.0));
        assert_eq!(server.wm_output_workarea(outputs[1],false),(800,232,1000,768));
        assert!(server.wm_text_overlays.iter().any(|o|o.window==panel2&&o.text.starts_with(b"__topbar\t")));

        assert!(server.select_shell_output(820,210));
        assert_eq!(server.hit_test(820,210),WM_GNOME_PANEL);
        assert_eq!(server.windows[&WM_GNOME_PANEL.0].width,1000);
        assert_eq!(server.windows[&WM_TASKBAR.0].x,0);
        assert!(server.windows.contains_key(&SHELL_REPLICA_BASE));
        assert!(!server.windows.contains_key(&panel2.0));

        assert!(server.set_shell_monitors(true,true));
        assert_eq!(server.wm_taskbar_rect(),(800,968,1000,32));
        assert_eq!(server.wm_output_workarea(outputs[1],false),(800,232,1000,736));
        assert_eq!(server.hit_test(820,980),WM_TASKBAR);
        assert!(server.windows.contains_key(&(SHELL_REPLICA_BASE+1)));
        server.set_taskbar_configuration(true,0,true);
        server.update_taskbar_pointer(900,999);
        assert!(server.wm_taskbar_revealed,"each monitor has its own edge sensor");
        assert!(!server.windows[&(SHELL_REPLICA_BASE+1)].mapped);

        assert!(server.set_shell_monitors(false,false));
        assert_eq!(server.windows[&WM_GNOME_PANEL.0].x,0);
        assert_eq!(server.wm_output_workarea(outputs[1],false),outputs[1]);
        assert!(!server.windows.keys().any(|id|shell_replica_source(ResourceId(*id)).is_some()));
        assert!(server.set_shell_monitors(true,true));
        assert!(server.set_output_layout(800,600,vec![outputs[0]]));
        assert!(!server.windows.keys().any(|id|shell_replica_source(ResourceId(*id)).is_some()));
        assert_eq!(server.windows[&WM_GNOME_PANEL.0].x,0);
    }
}

impl DisplayServer {
    fn plan_x11_output_topology(&mut self,outputs:&[(u16,u16,u16,u16)],mapping:&[u32],reset_mask:u32)->Vec<(ResourceId,ResourceId,X11OutputHome,(u16,u16,u16,u16))> {
        self.x11_output_homes.retain(|id,home|{
            home.slot=mapping[home.slot as usize];
            self.managed_frames.contains_key(id)&&reset_mask&(1<<home.slot)==0
        });
        let mut moves=Vec::new();
        for (&id,&frame) in &self.managed_frames {
            let (Some(client),Some(window))=(self.windows.get(&id),self.windows.get(&frame.0)) else{continue;};
            let old=self.task_output(WmTaskTarget::X11(ResourceId(id))) as usize;
            let Some(&source)=self.wm_outputs.get(old) else{continue;};
            let slot=mapping[old];
            let current=X11OutputHome{slot,output:source,geometry:(window.x,window.y,client.width,client.height),automatic:self.auto_tile_restore.get(&id).copied(),fullscreen:self.restore_geometries.get(&id).copied(),maximized:self.maximized_geometries.get(&id).copied(),snapped:self.snapped_geometries.get(&id).copied()};
            if let Some(home)=self.x11_output_homes.get(&id).copied() {
                if let Some(&target)=outputs.get(home.slot as usize){self.x11_output_homes.remove(&id);moves.push((ResourceId(id),frame,home,target));continue;}
            } else if slot as usize>=outputs.len(){self.x11_output_homes.insert(id,current);}
            let target=outputs.get(slot as usize).copied().unwrap_or(outputs[0]);
            if source!=target{moves.push((ResourceId(id),frame,current,target));}
        }
        moves
    }

    fn apply_x11_output_topology(&mut self,moves:Vec<(ResourceId,ResourceId,X11OutputHome,(u16,u16,u16,u16))>) {
        for (client,frame,home,target) in moves {
            let (left,right,top,bottom)=self.managed_frame_insets(client);
            let area=self.wm_output_workarea(target,false);
            let fit=|value:(i16,i16,u16,u16)|{
                let width=value.2.min(area.2.saturating_sub(left+right)).max(1);
                let height=value.3.min(area.3.saturating_sub(top+bottom)).max(1);
                let x=(i32::from(value.0)-i32::from(home.output.0)+i32::from(target.0)).clamp(i32::from(area.0),i32::from(area.0)+i32::from(area.2.saturating_sub(width.saturating_add(left+right))));
                let y=(i32::from(value.1)-i32::from(home.output.1)+i32::from(target.1)).clamp(i32::from(area.1),i32::from(area.1)+i32::from(area.3.saturating_sub(height.saturating_add(top+bottom))));
                (clamp_i16(x),clamp_i16(y),width,height)
            };
            if let Some(value)=home.automatic{self.auto_tile_restore.insert(client.0,fit(value));}
            if let Some(value)=home.fullscreen{self.restore_geometries.insert(client.0,fit(value));}
            if let Some(value)=home.maximized{self.maximized_geometries.insert(client.0,fit(value));}
            if let Some(mut value)=home.snapped{value.restore_geometry=fit(value.restore_geometry);self.snapped_geometries.insert(client.0,value);}
            let (x,y,width,height)=if home.fullscreen.is_some(){(target.0 as i16,target.1 as i16,target.2.saturating_sub(left+right).max(1),target.3.saturating_sub(top+bottom).max(1))}
                else if home.maximized.is_some(){(area.0 as i16,area.1 as i16,area.2.saturating_sub(left+right).max(1),area.3.saturating_sub(top+bottom).max(1))}
                else if let Some(snap)=home.snapped{let (x,y,w,h)=self.wm_snap_geometry(snap.target,area);(x as i16,y as i16,w.saturating_sub(left+right).max(1),h.saturating_sub(top+bottom).max(1))}
                else{fit(home.geometry)};
            self.apply_managed_geometry(client,frame,x,y,width,height);
        }
    }

    pub fn set_shell_monitors(&mut self, topbar: bool, dock: bool) -> bool {
        if !self.config.native_window_manager { return false; }
        if self.wm_topbar_all_monitors == topbar && self.wm_dock_all_monitors == dock { return true; }
        self.wm_topbar_all_monitors = topbar;
        self.wm_dock_all_monitors = dock;
        self.refresh_gnome_panel_geometry();
        self.refresh_taskbar_geometry();
        self.relayout_wm_edge_windows();
        self.relayout_wm_menu();
        self.rebuild_scene();
        true
    }

    pub fn select_drag_output(&mut self, source:u32, x:i16, y:i16)->bool {
        if self.input_state&0x0100==0 || self.wm_painting_replicas || self.wm_chrome_pressed.is_some() || self.wm_taskbar_drag.is_some()
            || self.windows.get(&WM_MENU.0).is_some_and(|w|w.mapped)
            || self.windows.get(&WM_GNOME_MENU.0).is_some_and(|w|w.mapped){return false;}
        let Some(grab)=self.pointer_grab else{return false;};
        if self.windows.get(&source).and_then(|window|window.owner)!=Some(grab.owner)
            || self.keyboard_grab.is_some_and(|keyboard|keyboard.owner!=grab.owner){return false;}
        let Some(index)=self.wm_outputs.iter().position(|&(ox,oy,w,h)|i32::from(x)>=i32::from(ox)&&i32::from(y)>=i32::from(oy)&&i32::from(x)<i32::from(ox)+i32::from(w)&&i32::from(y)<i32::from(oy)+i32::from(h)) else{return false;};
        if self.tile_modal_blocked(self.output_workspace(index as u32)){return false;}
        self.activate_shell_output(index);
        true
    }

    pub fn select_shell_output(&mut self, x: i16, y: i16) -> bool {
        if self.wm_painting_replicas || self.wm_chrome_pressed.is_some() || self.wm_taskbar_drag.is_some()
            || self.input_state & 0x1f00 != 0
            || self.windows.get(&WM_MENU.0).is_some_and(|w| w.mapped)
            || self.windows.get(&WM_GNOME_MENU.0).is_some_and(|w| w.mapped) { return false; }
        let Some(index) = self.wm_outputs.iter().position(|&(ox,oy,w,h)|
            i32::from(x) >= i32::from(ox) && i32::from(x) < i32::from(ox)+i32::from(w)
            && i32::from(y) >= i32::from(oy) && i32::from(y) < i32::from(oy)+i32::from(h)) else { return false; };
        self.activate_shell_output(index)
    }

    fn activate_shell_output(&mut self,index:usize)->bool {
        if index == self.wm_shell_output { return false; }
        self.wm_shell_output = index;
        self.select_output_workspace(index as u32);
        self.wm_chrome_hover = None;
        if self.wm_topbar_all_monitors { self.refresh_gnome_panel_geometry(); }
        if self.wm_dock_all_monitors {
            self.wm_taskbar_revealed = !self.wm_taskbar_auto_hide;
            self.refresh_taskbar_geometry();
        }
        true
    }

    pub(super) fn paint_output_shell_replicas(&mut self, source: ResourceId) {
        if self.wm_painting_replicas || !matches!(source, WM_GNOME_PANEL | WM_TASKBAR) { return; }
        let enabled = if source == WM_GNOME_PANEL { self.wm_topbar_all_monitors } else { self.wm_dock_all_monitors };
        let active = if enabled { self.wm_shell_output } else { 0 };
        let slot = u32::from(source == WM_TASKBAR);
        let Some(original_window) = self.windows.get(&source.0).cloned() else { return; };
        self.wm_painting_replicas = true;
        let original_output = self.wm_shell_output;
        let original_hover = self.wm_chrome_hover.take();
        let original_texts = self.wm_text_overlays.iter().filter(|o| o.window == source).cloned().collect::<Vec<_>>();
        let original_surface = self.surfaces.remove(&source.0);
        for index in 0..16 {
            let id = ResourceId(SHELL_REPLICA_BASE + index as u32 * 2 + slot);
            self.wm_text_overlays.retain(|o| o.window != id);
            if !enabled || index == active || index >= self.wm_outputs.len()
                || self.wm_outputs[..index].contains(&self.wm_outputs[index]) {
                if self.windows.remove(&id.0).is_some() { self.surfaces.remove(&id.0); self.scene_remove(id); }
                continue;
            }
            self.wm_shell_output = index;
            let (x,y,width,height) = if source == WM_TASKBAR { self.wm_taskbar_rect() } else {
                let (x,y,w,h) = self.wm_panel_output_rect();
                let height = self.wm_gnome_panel_height.min(h);
                (x as i16, (y + if self.wm_gnome_panel_bottom { h-height } else { 0 }) as i16, w, height)
            };
            let mut replica = original_window.clone();
            replica.x=x; replica.y=y; replica.width=width; replica.height=height;
            if source == WM_TASKBAR && self.wm_taskbar_auto_hide { replica.mapped=false; }
            self.windows.insert(source.0, replica.clone());
            self.surfaces.insert(source.0, Surface::new(width.max(1),height.max(1),32,0));
            self.paint_wm_chrome(source);
            replica.id=id;
            self.windows.insert(id.0,replica);
            if let Some(surface) = self.surfaces.remove(&source.0) { self.surfaces.insert(id.0,surface); }
            for overlay in self.wm_text_overlays.iter_mut().filter(|o|o.window==source) { overlay.window=id; }
            self.scene_upsert(id);
            self.scene_damage(id);
        }
        self.wm_shell_output=original_output;
        self.wm_chrome_hover=original_hover;
        self.windows.insert(source.0,original_window);
        if let Some(surface)=original_surface { self.surfaces.insert(source.0,surface); }
        self.wm_text_overlays.retain(|o|o.window!=source);
        self.wm_text_overlays.extend(original_texts);
        self.wm_painting_replicas=false;
    }
    /// Desktop-owned physical output rectangles in normalized global coordinates.
    /// Empty topology retains the browser's single virtual output behavior.
    /// Reorder an unchanged physical inventory. Indices identify entries in the
    /// previous inventory, not resources or caller authorization.
    pub fn reorder_output_layout(&mut self, width: u16, height: u16,
        outputs: Vec<(u16,u16,u16,u16)>, new_to_old: &[u32]) -> bool {
        let count = self.wm_outputs.len();
        if count == 0 || count > 16 || outputs.len() != count || new_to_old.len() != count
            || width != self.config.width_px || height != self.config.height_px {
            return false;
        }
        let mut old_to_new = vec![u32::MAX; count];
        for (new, &old) in new_to_old.iter().enumerate() {
            let Some(slot) = old_to_new.get_mut(old as usize) else { return false; };
            if *slot != u32::MAX || outputs[new] != self.wm_outputs[old as usize] { return false; }
            *slot = new as u32;
        }
        // All checks precede mutation. This path only permutes accepted geometry.
        for home in self.x11_output_homes.values_mut(){home.slot=old_to_new.get(home.slot as usize).copied().unwrap_or(home.slot);}
        for layout in self.tile_layouts.values_mut() { layout.strip.remap_output_indices(&old_to_new); }
        self.output_workspaces = self.output_workspaces.iter().map(|(&index,&workspace)|
            (old_to_new.get(index as usize).copied().unwrap_or(index),workspace)).collect();
        self.wm_shell_output = old_to_new[self.wm_shell_output.min(count-1)] as usize;
        self.wm_primary_output = old_to_new[self.wm_primary_output.min(count-1)] as usize;
        self.set_output_layout(width,height,outputs)
    }

    /// A complete slot permutation covers both connected and remembered
    /// disconnected outputs. Validate the entire transaction before mutation.
    pub fn map_output_layout(&mut self,width:u16,height:u16,outputs:Vec<(u16,u16,u16,u16)>,old_to_new:&[u32],reset_mask:u32)->bool {
        if outputs.is_empty() || !valid_output_layout(width,height,&outputs) || old_to_new.len()!=16 || reset_mask>0xffff || (reset_mask>>outputs.len())!=0 {return false;}
        let mut seen=0u32;
        for &index in old_to_new {if index>=16 || seen&(1<<index)!=0{return false;}seen|=1<<index;}
        let moves=self.plan_x11_output_topology(&outputs,old_to_new,reset_mask);
        for layout in self.tile_layouts.values_mut(){layout.strip.remap_output_indices(old_to_new);}
        self.output_workspaces=self.output_workspaces.iter().filter_map(|(&old,&workspace)|old_to_new.get(old as usize).map(|&index|(index,workspace))).filter(|(index,_)|reset_mask&(1<<index)==0).collect();
        self.wm_shell_output=old_to_new[self.wm_shell_output.min(15)] as usize;
        self.wm_primary_output=old_to_new[self.wm_primary_output.min(15)] as usize;
        if self.wm_shell_output>=outputs.len(){self.wm_shell_output=0;}
        if self.wm_primary_output>=outputs.len(){self.wm_primary_output=0;}
        let changed=self.set_output_layout(width,height,outputs);
        if changed {self.apply_x11_output_topology(moves);self.select_output_workspace(self.wm_shell_output as u32);if self.scrolling_workspaces{self.refresh_scrolling_workspaces();self.apply_workspace_visibility();}}
        changed
    }

    /// Keep surviving output indices and their remembered disconnected slots
    /// together when the host removes entries from a named inventory.
    pub fn subset_output_layout(&mut self, width: u16, height: u16,
        outputs: Vec<(u16,u16,u16,u16)>, new_to_old: &[u32]) -> bool {
        let count=self.wm_outputs.len();
        if outputs.is_empty() || outputs.len()>=count || new_to_old.len()!=outputs.len()
            || !valid_output_layout(width,height,&outputs) { return false; }
        let mut order=Vec::new();
        for &old in new_to_old {
            if old as usize>=count || order.contains(&old) {return false;}
            order.push(old);
        }
        for old in 0..count as u32 {if !order.contains(&old){order.push(old);}}
        let previous=self.wm_outputs.clone();
        let reordered=order.iter().map(|&old|previous[old as usize]).collect();
        if !self.reorder_output_layout(self.config.width_px,self.config.height_px,reordered,&order){return false;}
        if self.wm_shell_output>=outputs.len(){self.wm_shell_output=0;}
        if self.wm_primary_output>=outputs.len(){self.wm_primary_output=0;}
        self.set_output_layout(width,height,outputs)
    }

    pub fn set_output_layout(
        &mut self,
        width: u16,
        height: u16,
        outputs: Vec<(u16, u16, u16, u16)>,
    ) -> bool {
        if !valid_output_layout(width,height,&outputs) { return false; }
        let resized = width != self.config.width_px || height != self.config.height_px;
        if !resized && self.wm_outputs == outputs {
            return true;
        }
        let previous = std::mem::replace(&mut self.wm_outputs, outputs);
        self.wm_shell_output = self.wm_shell_output.min(self.wm_outputs.len().saturating_sub(1));
        self.wm_primary_output = self.wm_primary_output.min(self.wm_outputs.len().saturating_sub(1));
        if !self.resize_output(width, height) {
            self.wm_outputs = previous;
            return false;
        }
        self.refresh_gnome_panel_geometry();
        self.refresh_taskbar_geometry();
        self.relayout_wm_menu();
        // Recover floating windows whose title bar was on a removed output or
        // now lies in a gap. Keep reachable windows where the user placed them.
        let stranded = self.managed_frames.iter().filter_map(|(&client, &frame)| {
            let window = self.windows.get(&frame.0)?;
            let reachable = self.wm_outputs.iter().any(|&(x,y,w,h)| {
                let left = i32::from(window.x).max(i32::from(x));
                let right = (i32::from(window.x)+i32::from(window.width)).min(i32::from(x)+i32::from(w));
                right-left >= i32::from(window.width.min(64)) &&
                    i32::from(window.y) >= i32::from(y) && i32::from(window.y) < i32::from(y)+i32::from(h)
            });
            (!reachable && !self.wm_outputs.is_empty()).then_some((ResourceId(client), frame, window.width, window.height))
        }).collect::<Vec<_>>();
        for (client,frame,width,height) in stranded {
            let (x,y,w,h) = self.wm_window_output_rect(frame,false);
            let (left,right,top,bottom) = self.managed_frame_insets(client);
            self.apply_managed_geometry(client,frame,x as i16,y as i16,
                width.min(w).saturating_sub(left+right),height.min(h).saturating_sub(top+bottom));
        }
        self.select_output_workspace(self.wm_shell_output as u32);
        if self.scrolling_workspaces {self.refresh_scrolling_workspaces();self.apply_workspace_visibility();}
        // Same virtual dimensions can still describe a different arrangement.
        if !resized {
            self.relayout_wm_edge_windows();
            self.rebuild_scene();
        }
        true
    }

    pub(super) fn wm_window_output_rect(
        &self,
        frame: ResourceId,
        fullscreen: bool,
    ) -> (u16, u16, u16, u16) {
        let area = if fullscreen {
            (0, 0, self.config.width_px, self.config.height_px)
        } else {
            self.wm_workarea_rect()
        };
        let Some(first) = self.wm_outputs.first().copied() else {
            return area;
        };
        let window = self.windows.get(&frame.0);
        let overlap = |&(x, y, w, h): &(u16, u16, u16, u16)| -> i64 {
            let Some(window) = window else {
                return 0;
            };
            let left = i32::from(x).max(i32::from(window.x));
            let top = i32::from(y).max(i32::from(window.y));
            let right =
                (i32::from(x) + i32::from(w)).min(i32::from(window.x) + i32::from(window.width));
            let bottom =
                (i32::from(y) + i32::from(h)).min(i32::from(window.y) + i32::from(window.height));
            i64::from((right - left).max(0)) * i64::from((bottom - top).max(0))
        };
        let chosen = self.wm_outputs.iter().copied().fold(first, |best, item| {
            if overlap(&item) > overlap(&best) {
                item
            } else {
                best
            }
        });
        self.wm_output_workarea(chosen, fullscreen)
    }

    pub(super) fn wm_output_workarea(&self, chosen: (u16,u16,u16,u16), fullscreen: bool) -> (u16,u16,u16,u16) {
        let (x, y, w, h) = chosen;
        if fullscreen { return chosen; }
        let dock = chosen == self.wm_primary_rect() || self.wm_dock_all_monitors;
        let topbar = chosen == self.wm_primary_rect() || self.wm_topbar_all_monitors;
        let taskbar_height = if dock { self.wm_reserved_taskbar_height().min(h) } else { 0 };
        let taskbar_width = if dock { self.wm_reserved_taskbar_width().min(w) } else { 0 };
        let panel = if topbar { self.wm_reserved_gnome_panel_height().min(h.saturating_sub(taskbar_height)) } else { 0 };
        (x.saturating_add(if self.wm_taskbar_edge == WmTaskbarEdge::Left { taskbar_width } else { 0 }),
         y.saturating_add(if self.wm_gnome_panel_bottom { 0 } else { panel }).saturating_add(if self.wm_taskbar_edge == WmTaskbarEdge::Top { taskbar_height } else { 0 }),
         w.saturating_sub(taskbar_width), h.saturating_sub(panel + taskbar_height))
    }

    pub(super) fn relayout_wm_fullscreen_outputs(&mut self) {
        // Keep browser behavior unchanged; this path is enabled by desktop topology.
        if self.wm_outputs.is_empty() {
            return;
        }
        let windows = self
            .restore_geometries
            .keys()
            .filter_map(|client| {
                self.managed_frames
                    .get(client)
                    .map(|frame| (ResourceId(*client), *frame))
            })
            .collect::<Vec<_>>();
        for (client, frame) in windows {
            let (x, y, w, h) = self.wm_window_output_rect(frame, true);
            self.apply_managed_geometry(
                client,
                frame,
                x as i16,
                y as i16,
                w.saturating_sub(WM_BORDER_WIDTH * 2),
                h.saturating_sub(WM_TITLE_HEIGHT)
                    .saturating_sub(WM_BORDER_WIDTH),
            );
        }
    }
}

#[cfg(test)]
mod inventory_reorder_tests {
    use super::*;
    #[test]
    fn remembered_slots_reconnect_in_arbitrary_order_and_invalid_maps_are_atomic() {
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let original=vec![(0,0,800,700),(800,0,800,700)];
        assert!(s.set_output_layout(1600,700,original.clone()));s.set_scrolling_workspaces(true);
        s.output_workspaces.insert(0,3);s.output_workspaces.insert(1,7);s.wm_shell_output=1;s.current_workspace=7;
        let mut mapping:Vec<u32>=(0..16).collect();mapping.swap(0,1);
        let mut bad=mapping.clone();bad[2]=bad[0];
        assert!(!s.map_output_layout(800,700,vec![(0,0,800,700)],&bad,0));
        assert!(!s.map_output_layout(800,700,vec![(0,0,800,700)],&mapping,2));
        assert_eq!(s.wm_outputs,original);assert_eq!(s.output_workspace(0),3);assert_eq!(s.active_output(),1);
        assert!(s.map_output_layout(800,700,vec![(0,0,800,700)],&mapping,0));assert_eq!(s.output_workspace(0),7);
        assert!(s.map_output_layout(1600,700,original,&mapping,0));
        assert_eq!(s.output_workspace(0),3);assert_eq!(s.output_workspace(1),7);assert_eq!(s.active_output(),1);assert_eq!(s.current_workspace(),7);
    }
    #[test]
    fn removing_first_output_preserves_survivor_workspace_and_rejects_invalid_topology() {
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let original=vec![(0,0,800,700),(800,0,800,700)];
        assert!(s.set_output_layout(1600,700,original.clone()));s.set_scrolling_workspaces(true);
        s.output_workspaces.insert(0,3);s.output_workspaces.insert(1,7);s.wm_shell_output=1;s.current_workspace=7;
        assert!(!s.subset_output_layout(800,700,vec![(0,0,900,700)],&[1]));
        assert!(!s.subset_output_layout(800,700,vec![(0,0,800,700)],&[2]));
        assert_eq!(s.wm_outputs,original);assert_eq!(s.output_workspace(0),3);assert_eq!(s.active_output(),1);
        assert!(s.subset_output_layout(800,700,vec![(0,0,800,700)],&[1]));
        assert_eq!(s.output_workspace(0),7);assert_eq!(s.output_workspace(1),3);
        assert_eq!(s.active_output(),0);assert_eq!(s.current_workspace(),7);
    }
    #[test]
    fn inventory_reorder_preserves_selection_and_rejects_invalid_permutations_atomically() {
        let mut s=DisplayServer::new(DisplayConfig{width_px:1600,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let original=vec![(0,0,800,700),(800,0,800,700)];
        assert!(s.set_output_layout(1600,700,original.clone()));
        s.set_scrolling_workspaces(true);
        s.output_workspaces.insert(0,3);s.output_workspaces.insert(1,7);
        s.wm_shell_output=1;s.current_workspace=7;
        for mapping in [vec![0,0],vec![2,0],vec![u32::MAX,0]] {
            assert!(!s.reorder_output_layout(1600,700,original.clone(),&mapping));
            assert_eq!(s.wm_outputs,original);assert_eq!(s.output_workspace(0),3);assert_eq!(s.output_workspace(1),7);assert_eq!(s.active_output(),1);
        }
        let reversed=vec![original[1],original[0]];
        assert!(!s.reorder_output_layout(1600,700,reversed.clone(),&[0,1]));
        assert!(s.reorder_output_layout(1600,700,reversed,&[1,0]));
        assert_eq!(s.output_workspace(0),7);assert_eq!(s.output_workspace(1),3);
        assert_eq!(s.active_output(),0);assert_eq!(s.current_workspace,7);
        assert_eq!(s.wm_primary_rect(),original[0]);
    }
}

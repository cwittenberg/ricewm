//! Navigation operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub fn navigation_begin(&mut self) { self.navigation_native.clear(); }
    pub fn keyboard_grabbed(&self) -> bool { self.keyboard_grab.is_some() }
    pub fn pointer_grabbed(&self) -> bool { self.pointer_grab.is_some() }
    pub fn tile_overflow(&self) -> bool {
        let (x,y,width,height)=self.wm_workarea_rect();
        self.auto_tile_enabled&&self.tile_layouts.get(&self.current_workspace).is_some_and(|layout|{
                    layout.overflows(TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    })
                })
    }
    pub fn tile_move_request_field(&self, field: u32) -> u32 {
        let Some((client,x,y,cancel))=self.tile_move_request else {return 0;};
        match field {0=>client,1=>x as i32 as u32,2=>y as i32 as u32,3=>u32::from(cancel),_=>0,
        }
    }
    pub fn clear_tile_move_request(&mut self) {self.tile_move_request=None;}
    pub fn release_tile_pointer(&mut self, x:i16, y:i16, state:u16) {
        // The layout owner consumes this release. Keep QueryPointer state
        // current without delivering an unmatched guest ButtonRelease.
        self.pointer_x=x;self.pointer_y=y;self.input_state=state & !0x0100;
    }
    pub fn tile_input_blocked(&self) -> bool {
        self.pointer_grabbed()||self.keyboard_grabbed()||self.tile_modal_blocked(self.current_workspace)
    }
    pub(super) fn tile_modal_blocked(&self,workspace:u32)->bool {
        self.managed_frames.iter().any(|(id,frame)|
            {
                self.window_workspaces.get(id)==Some(&workspace)&&
            !self.minimized_windows.contains(id)&&self.windows.get(&frame.0).is_some_and(|window|window.mapped)&&
            self.managed_client_is_modal(ResourceId(*id))})
    }

    pub fn navigation_native_frame(&mut self, id: u16, x: i32, y: i32, width: u32, height: u32) {
        if id != 0 && width > 0 && height > 0 && self.navigation_native.len() < 128 {
            self.navigation_native.insert(id, (x, y, width, height));
        }
    }

    /// Find a visible neighbor to the left/right/up/down (0..3).
    pub(super) fn directional_neighbor(&self, direction: u32, tiled_only: bool) -> Option<WmTaskTarget> {
        self.directional_neighbor_from(self.active_wm_task_target()?, direction, tiled_only)
    }
    pub(super) fn directional_neighbor_from(&self, active: WmTaskTarget, direction: u32, tiled_only: bool) -> Option<WmTaskTarget> {
        if direction > 3 { return None; }
        if self.auto_tile_enabled {
            if let Some(layout)=self.tile_layouts.get(&self.current_workspace).filter(|layout|layout.scrolling) {
                return layout.strip.neighbor(active,direction);
            }
        }
        if self.tile_overflow() {
            let (x,y,width,height)=self.wm_workarea_rect();
            let order=self.tile_layouts.get(&self.current_workspace)?.rectangles(TileRect{x:i32::from(x),y:i32::from(y),width,height,
                });
            if let Some(index)=order.iter().position(|(target,_)|*target==active) {
                if order.len()<2 {return None;}
                return Some(order[(index+if direction==0||direction==2 {order.len()-1}else{1})%order.len()].0,
                );
            }
        }
        let mut candidates = Vec::new();
        for task in &self.native_tasks {
            if task.workspace == self.current_workspace && !task.minimized {
                if let Some(&(x,y,w,h)) = self.navigation_native.get(&task.id) {
                    candidates.push((WmTaskTarget::Native(task.id), (i64::from(x)*2+i64::from(w), i64::from(y)*2+i64::from(h),
                        ),
                    ));
                }
            }
        }
        for (&id, &frame) in &self.managed_frames {
            if self.window_workspaces.get(&id) != Some(&self.current_workspace) || self.minimized_windows.contains(&id) || self.parked_managed_windows.contains(&id) { continue; }
            if let Some(w) = self.windows.get(&frame.0).filter(|w| w.mapped) {
                candidates.push((WmTaskTarget::X11(ResourceId(id)), (i64::from(w.x)*2+i64::from(w.width), i64::from(w.y)*2+i64::from(w.height),
                    ),
                ));
            }
        }
        if tiled_only {
            let layout = self.tile_layouts.get(&self.current_workspace)?;
            candidates.retain(|(target,_)| layout.contains(*target));
        }
        let &(_, (ax,ay)) = candidates.iter().find(|(target,_)| *target == active)?;
        candidates.iter().filter_map(|&(target,(x,y))| {
            let (forward,side) = match direction {0=>(ax-x,y-ay),1=>(x-ax,y-ay),2=>(ay-y,x-ax),_=>(y-ay,x-ax),
                };
            (target != active && forward > 0).then_some(((i128::from(forward).pow(2)+i128::from(side).pow(2),side.abs(),
                    ),target,
                ))
        }).min_by_key(|(score,_)| *score).map(|(_,target)| target)
    }

    pub fn swap_direction(&mut self, direction: u32) -> bool {
        if !self.auto_tile_enabled { return false; }
        let Some(active) = self.active_wm_task_target() else { return false; };
        let Some(neighbor) = self.directional_neighbor(direction, true) else { return false; };
        self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|layout.swap(active,neighbor))
    }
    /// Read-only state for the six native titlebar tile controls.
    pub fn tile_control_available(&self, kind: u32, id: u32, action: u32) -> bool {
        if !self.auto_tile_enabled || self.tile_overflow() {return false;}
        let Some(target)=self.live_tile_target(kind,id) else {return false;};
        let (x,y,width,height)=self.wm_workarea_rect();
        let area=TileRect{x:i32::from(x),y:i32::from(y),width,height};
        if action < 4 {
            let Some(layout)=self.tile_layouts.get(&self.current_workspace) else {return false;};
            if layout.scrolling {return layout.strip.neighbor(target,action).is_some();}
            let cells=layout.rectangles(area);
            let Some((_,active))=cells.iter().find(|(candidate,_)|*candidate==target) else {return false;};
            let ax=i64::from(active.x)*2+i64::from(active.width);
            let ay=i64::from(active.y)*2+i64::from(active.height);
            return cells.iter().any(|(candidate,rect)|{
                if *candidate==target {return false;}
                let dx=i64::from(rect.x)*2+i64::from(rect.width)-ax;
                let dy=i64::from(rect.y)*2+i64::from(rect.height)-ay;
                match action {0=>dx<0,1=>dx>0,2=>dy<0,_=>dy>0}
            });
        }
        if action > 5 {return false;}
        self.tile_layouts.get(&self.current_workspace).is_some_and(|layout|{
            let mut probe=layout.clone();
            probe.resize(target,TileAxis::Horizontal,if action==4 {32}else{-32},area)
        })
    }
    pub fn insert_direction(&mut self, direction: u32) -> bool {
        if !self.auto_tile_enabled {return false;}
        let Some(active)=self.active_wm_task_target() else {return false;};
        let Some(neighbor)=self.directional_neighbor(direction,true) else {return false;};
        let (x,y,width,height)=self.wm_workarea_rect();
        let inserted=self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|{
                layout.insert_beside(active,neighbor,direction,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            });
        if inserted {if let WmTaskTarget::X11(id)=active{self.x11_output_homes.remove(&id.0);}}
        inserted
    }
    pub(super) fn live_tile_target(&self, kind: u32, id: u32) -> Option<WmTaskTarget> {
        let target=match kind {
            0 if self.managed_frames.contains_key(&id)&&self.window_workspaces.get(&id)==Some(&self.current_workspace)&&!self.minimized_windows.contains(&id)=>{
                WmTaskTarget::X11(ResourceId(id))}
            1 if self.native_tasks.iter().any(|task|{
                u32::from(task.id)==id&&task.workspace==self.current_workspace&&!task.minimized}) =>
            {
                WmTaskTarget::Native(u16::try_from(id).ok()?)}
            _=>return None,
        };
        self.tile_layouts.get(&self.current_workspace)?.contains(target).then_some(target)
    }
    pub fn tile_at_field(&self, px: i32, py: i32, field: u32) -> u32 {
        if !self.auto_tile_enabled||self.tile_overflow() {return 0;}
        let (x,y,width,height)=self.wm_workarea_rect();
        let Some(layout)=self.tile_layouts.get(&self.current_workspace) else {return 0;};
        for (target,rect) in layout.rectangles(TileRect{x:i32::from(x),y:i32::from(y),width,height,
        }) {
            if let Some(clip)=self.scrolling_clip(target){if px<clip.x||py<clip.y||px>=clip.x+i32::from(clip.width)||py>=clip.y+i32::from(clip.height){continue;}}
            if px<i32::from(rect.x)||py<i32::from(rect.y)||px>=i32::from(rect.x)+i32::from(rect.width)||py>=i32::from(rect.y)+i32::from(rect.height) {continue;}
            let (kind,id)=match target {WmTaskTarget::X11(id)=>(0,id.0),WmTaskTarget::Native(id)=>(1,u32::from(id)),_=>continue,
            };
            if self.live_tile_target(kind,id).is_none() {return 0;}
            if kind==0 {
                let (Ok(hx),Ok(hy))=(i16::try_from(px),i16::try_from(py)) else {return 0;};
                if self.managed_client_ancestor(self.hit_test(hx,hy))!=Some(ResourceId(id)) {return 0;}
            }
            return match field {0=>kind,1=>id,2=>rect.x as u32,3=>rect.y as u32,4=>u32::from(rect.width),5=>u32::from(rect.height),_=>0,
            };
        }
        0
    }
    /// Only compositor-owned title text starts an insertion gesture. Icons,
    /// controls, resize edges, modal owners and client content retain their input.
    pub fn tiled_x11_title_at(&self, x: i32, y: i32) -> u32 {
        if !self.auto_tile_enabled||self.tile_input_blocked() {return 0;}
        let (Ok(px),Ok(py))=(i16::try_from(x),i16::try_from(y)) else {return 0;};
        let frame=self.hit_test(px,py);
        let Some(client)=self.frame_clients.get(&frame.0).copied() else {return 0;};
        if self.live_tile_target(0,client.0).is_none()||self.topmost_managed_modal_transient(client).is_some()||self.wm_chrome_hit(frame,px,py).is_some() {return 0;}
        let (_,_,title_height,_)=self.managed_frame_insets(client);
        let (ox,oy)=self.window_origin(frame);
        let Some(window)=self.windows.get(&frame.0) else {return 0;};
        if x-ox<WM_RESIZE_HIT_WIDTH||x-ox>=i32::from(window.width)-WM_RESIZE_HIT_WIDTH||y-oy<WM_RESIZE_HIT_WIDTH||y-oy>=i32::from(title_height) {return 0;}
        client.0
    }
    pub fn activate_tiled_x11(&mut self,id:u32)->bool {
        if self.tile_input_blocked()||self.live_tile_target(0,id).is_none()||self.topmost_managed_modal_transient(ResourceId(id)).is_some() {return false;}
        self.activate_managed_window(ResourceId(id));true
    }
    pub fn preview_tile_insertion(&mut self, source_kind: u32, source_id: u32, target_kind: u32, target_id: u32, direction: u32,
    ) -> bool {
        self.tile_insertion_preview=None;
        if !self.auto_tile_enabled||self.tile_input_blocked() {return false;}
        let (Some(source),Some(target))=(self.live_tile_target(source_kind,source_id),self.live_tile_target(target_kind,target_id),
        ) else {return false;};
        let (x,y,width,height)=self.wm_workarea_rect();
        let preview=self.tile_layouts.get(&self.current_workspace).and_then(|layout|{
                layout.insertion_preview(source,target,direction,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            });
        self.tile_insertion_preview=preview.map(|rect|(self.current_workspace,rect));
        preview.is_some()
    }
    pub fn tile_insertion_preview_field(&self, field: u32) -> u32 {
        let Some((workspace,rect))=self.tile_insertion_preview else {return 0;};
        if workspace!=self.current_workspace||!self.auto_tile_enabled {return 0;}
        match field {0=>rect.x as u32,1=>rect.y as u32,2=>u32::from(rect.width),3=>u32::from(rect.height),_=>0}
    }
    pub fn insert_tile(&mut self, source_kind: u32, source_id: u32, target_kind: u32, target_id: u32, direction: u32,
    ) -> bool {
        self.tile_insertion_preview=None;
        if !self.auto_tile_enabled||self.tile_input_blocked() {return false;}
        let (Some(source),Some(target))=(self.live_tile_target(source_kind,source_id),self.live_tile_target(target_kind,target_id),
        ) else {return false;};
        let (x,y,width,height)=self.wm_workarea_rect();
        let inserted=self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|{
                layout.insert_beside(source,target,direction,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            });
        if inserted {if let WmTaskTarget::X11(id)=source{self.x11_output_homes.remove(&id.0);}}
        inserted
    }

    /// Resize one split by 32 logical pixels. Operation is grow/shrink width,
    /// grow/shrink height (0..3). Floating geometry and focus remain unchanged.
    pub fn resize_tile(&mut self, operation: u32) -> bool {
        if !self.auto_tile_enabled || operation > 6 {return false;}
        let Some(active)=self.active_wm_task_target() else {return false;};
        let (x,y,width,height)=self.wm_workarea_rect();
        if operation>=4 {
            return self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|
                layout.scrolling_preset(active,operation,TileRect{x:i32::from(x),y:i32::from(y),width,height}));
        }
        let axis=if operation<2 {TileAxis::Horizontal}else{TileAxis::Vertical};
        let delta=if operation%2==0 {32}else{-32};
        self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|{
                layout.resize(active,axis,delta,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            })
    }
    pub fn tile_divider_at(&self, x: i32, y: i32) -> u32 {
        if !self.auto_tile_enabled || self.tile_input_blocked() {return 0;}
        let (ax,ay,width,height)=self.wm_workarea_rect();
        self.tile_layouts.get(&self.current_workspace).map_or(0,|layout|{
                layout.divider_at(x,y,TileRect{x:i32::from(ax),y:i32::from(ay),width,height,
                    },
                )
            })
    }
    pub fn tile_divider_position(&self, token: u32) -> u32 {
        let (x,y,width,height)=self.wm_workarea_rect();
        self.tile_layouts.get(&self.current_workspace).and_then(|layout|{
                layout.divider_position(token,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            }).map_or(u32::MAX,|value|value as u32)
    }
    pub fn move_tile_divider(&mut self, token: u32, position: i32) -> bool {
        if !self.auto_tile_enabled || self.tile_input_blocked() {return false;}
        let (x,y,width,height)=self.wm_workarea_rect();
        self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|{
                layout.move_divider(token,position,TileRect{x:i32::from(x),y:i32::from(y),width,height,
                    },
                )
            })
    }

    /// Returns a native task id, 65536 for an activated X11 client, or zero
    /// when there is no neighbor. The native owner applies the returned focus.
    pub fn focus_direction(&mut self, direction: u32) -> u32 {
        match self.directional_neighbor(direction, false) {
            Some(WmTaskTarget::Native(id)) => u32::from(id),
            Some(WmTaskTarget::X11(client)) => {
                let client = self.topmost_managed_modal_transient(client).unwrap_or(client);
                self.activate_managed_window(client);
                65536
            }
            _ => 0,
        }
    }
}

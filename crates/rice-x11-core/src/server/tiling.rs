//! Tiling operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub fn begin_auto_tile(&mut self, enabled: bool) {
        self.auto_tile_workspace=self.current_workspace;
        self.auto_tile_previous_enabled = self.auto_tile_enabled;
        self.auto_tile_enabled = enabled;
        self.auto_tile_focus=None;
        self.auto_tile_native.clear();
        self.auto_tile_positions.clear();
        self.auto_tile_fullscreens.clear();
        self.auto_tile_clips.clear();
        self.auto_tile_minimums.clear();
        self.auto_tile_rects.clear();
    }
    pub fn set_auto_tile_workspace(&mut self,workspace:u32) {
        // An invalid context poisons this plan instead of reusing the last workspace.
        self.auto_tile_workspace=if workspace<self.workspace_count {workspace}else{self.auto_tile_enabled=self.auto_tile_previous_enabled;u32::MAX};
    }
    pub fn auto_tile_clip_count(&self)->u32 {self.auto_tile_clips.len() as u32}
    pub fn auto_tile_clip_field(&self,index:u32,field:u32)->u32 {
        self.auto_tile_clips.get(index as usize).map_or(0,|(kind,id,r)|match field{0=>*kind,1=>*id,2=>r.x as u32,3=>r.y as u32,4=>u32::from(r.width),5=>u32::from(r.height),_=>0})
    }
    pub fn set_auto_tile_position(&mut self,id:u16,x:i32,y:i32) {
        if self.auto_tile_workspace>=self.workspace_count || !self.auto_tile_native.contains(&id) {return;}
        self.set_native_output_position(id,x,y);
        self.auto_tile_positions.retain(|(candidate,_,_)|*candidate!=id);self.auto_tile_positions.push((id,x,y));
    }
    pub fn set_auto_tile_fullscreen(&mut self,id:u16) {if self.auto_tile_workspace<self.workspace_count && self.auto_tile_native.contains(&id) && !self.auto_tile_fullscreens.contains(&id){self.auto_tile_fullscreens.push(id);}}
    pub(super) fn tile_fullscreen(&self,target:WmTaskTarget)->bool {match target{WmTaskTarget::Native(id)=>self.auto_tile_fullscreens.contains(&id),WmTaskTarget::X11(id)=>self.restore_geometries.contains_key(&id.0),_=>false}}
    pub(super) fn scrolling_clip(&self,target:WmTaskTarget)->Option<TileRect> {
        let workspace=match target {WmTaskTarget::X11(id)=>self.window_workspaces.get(&id.0).copied(),WmTaskTarget::Native(id)=>self.native_tasks.iter().find(|t|t.id==id).map(|t|t.workspace),_=>None}.unwrap_or(self.auto_tile_workspace);
        if !self.tiled_workspaces.contains(&workspace){return None;}
        let layout=self.tile_layouts.get(&workspace)?;
        if !layout.scrolling{return None;}
        let clip=layout.strip.clip(target)?;
        if self.tile_fullscreen(target){
            let physical=self.wm_outputs.iter().copied().find(|&output|{let (x,y,w,h)=self.wm_output_workarea(output,false);clip==TileRect{x:i32::from(x),y:i32::from(y),width:w,height:h}}).unwrap_or((0,0,self.config.width_px,self.config.height_px));
            return Some(TileRect{x:i32::from(physical.0),y:i32::from(physical.1),width:physical.2,height:physical.3});
        }
        Some(clip)
    }
    pub fn scrolling_clip_field(&self,kind:u32,id:u32,field:u32)->u32 {
        let target=match (kind,id) { (0,1..)=>WmTaskTarget::X11(ResourceId(id)),(1,1..=65535)=>WmTaskTarget::Native(id as u16),_=>return 0 };
        self.scrolling_clip(target).map_or(0,|r|match field{0=>r.x as u32,1=>r.y as u32,2=>u32::from(r.width),3=>u32::from(r.height),_=>0})
    }
    pub fn scroll_tiles_at(&mut self,delta:i32,x:i32,y:i32)->bool {
        if !self.tiled_workspaces.contains(&self.current_workspace)||self.tile_input_blocked(){return false;}
        self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|layout.scrolling&&layout.strip.pan_at(delta,x,y))
    }
    /// Trusted shell edge navigation for an ongoing application drag. The
    /// source must belong to the client that still owns the held pointer grab.
    /// This never releases grabs or synthesizes pointer/drop events.
    pub fn scroll_drag_tiles_at(&mut self,source:u32,delta:i32,x:i32,y:i32)->bool {
        if delta==0 || delta.unsigned_abs()>4096 || self.input_state&0x0100==0 {return false;}
        let Some(grab)=self.pointer_grab else{return false;};
        if self.windows.get(&source).and_then(|window|window.owner)!=Some(grab.owner)
            || self.keyboard_grab.is_some_and(|keyboard|keyboard.owner!=grab.owner) {return false;}
        let workspace=if self.wm_outputs.is_empty(){self.current_workspace}else{
            let Some(index)=self.wm_outputs.iter().position(|&(ox,oy,w,h)|x>=i32::from(ox)&&y>=i32::from(oy)&&x<i32::from(ox)+i32::from(w)&&y<i32::from(oy)+i32::from(h)) else{return false;};
            self.output_workspace(index as u32)
        };
        // The last layout plan can belong to a different, floating monitor.
        // Authorize against the destination workspace's committed tiling state.
        if !self.tiled_workspaces.contains(&workspace)||self.tile_modal_blocked(workspace){return false;}
        self.tile_layouts.get_mut(&workspace).is_some_and(|layout|layout.scrolling&&layout.strip.pan_at(delta,x,y))
    }
    pub fn set_auto_tile_focus(&mut self,id:u16) {self.auto_tile_focus=(id!=0).then_some(WmTaskTarget::Native(id));}
    pub fn set_auto_tile_scrolling(&mut self, enabled: bool) {
        if self.auto_tile_workspace>=self.workspace_count {return;}
        self.tile_layouts.entry(self.auto_tile_workspace).or_default().scrolling=enabled;
    }
    pub fn scroll_tiles(&mut self, delta: i32) -> bool {
        if !self.tiled_workspaces.contains(&self.current_workspace) || self.tile_input_blocked() {return false;}
        let (x,y,width,height)=self.wm_workarea_rect();
        self.tile_layouts.get_mut(&self.current_workspace).is_some_and(|layout|layout.scrolling && layout.strip.pan_at(delta,i32::from(x)+i32::from(width)/2,i32::from(y)+i32::from(height)/2))
    }
    pub fn set_auto_tile_master(&mut self, master: bool) {
        if self.auto_tile_workspace>=self.workspace_count {return;}
        self.tile_layouts.entry(self.auto_tile_workspace).or_default().set_master(master);
    }
    /// Transfer format: version, identity count, (kind,id) pairs, tree words.
    /// Hosts must replace this session-local identity table before restoring
    /// into another session. Tree leaves reference table indices, not raw IDs.
    pub fn capture_workspace_layout(&mut self, workspace: u32) -> usize {
        self.tile_snapshot_buffer.clear();
        if workspace>=self.workspace_count {return 0;}
        let Some(layout)=self.tile_layouts.get(&workspace) else {return 0;};
        let members: Vec<_>=self.managed_frames.keys().filter(|id|
            self.window_workspaces.get(id)==Some(&workspace)
        ).map(|id|WmTaskTarget::X11(ResourceId(*id))).chain(self.native_tasks.iter()
            .filter(|task|task.workspace==workspace).map(|task|WmTaskTarget::Native(task.id)),
            ).collect();
        let mut snapshot=layout.clone();
        let (x,y,width,height)=self.wm_workarea_rect();let area=TileRect{x:i32::from(x),y:i32::from(y),width,height,
        };
        snapshot.retain_live(&members,area);
        snapshot.set_master(false);
        // Canonical snapshots expose Split first; selecting Master later still
        // restores its independent tree through the ordinary arrangement API.
        let mut used=Vec::new();
        let tree=snapshot.snapshot(|id| {
            if let Some(index)=used.iter().position(|member|*member==id) {index as u32}
            else {used.push(id);(used.len()-1) as u32}
        });
        if used.len()>256 {return 0;}
        if TileLayout::from_snapshot(&tree,|index|used.get(index as usize).copied()).is_none() {return 0;}
        let mut words=vec![1,used.len() as u32];
        for member in &used {match member {
            WmTaskTarget::X11(id)=>words.extend([0,id.0]),
            WmTaskTarget::Native(id)=>words.extend([1,u32::from(*id)]),
            WmTaskTarget::Group(_)=>unreachable!(),
        }}
        words.extend(tree);
        self.tile_snapshot_buffer=words;
        self.tile_snapshot_buffer.len()
    }
    pub fn workspace_layout_word(&self,index:usize)->u32 {self.tile_snapshot_buffer.get(index).copied().unwrap_or(0)}
    pub fn prepare_layout_identity(&mut self,id:u32)->usize {
        self.tile_identity_buffer.clear();
        if !self.managed_frames.contains_key(&id) {return 0;}
        let class=self.atoms.intern("WM_CLASS",false);
        let role=self.atoms.intern("WM_WINDOW_ROLE",false);
        let Some(window)=self.windows.get(&id) else {return 0;};
        let class=window.properties.get(&class).map(|p|p.data.clone()).unwrap_or_default();
        if class.is_empty() {return 0;}
        let role=window.properties.get(&role).map(|p|p.data.clone()).unwrap_or_default();
        let title=self.wm_window_title(ResourceId(id));
        if class.len()+role.len()+title.len()>768 {return 0;}
        for part in [class,role,title] {self.tile_identity_buffer.extend((part.len() as u32).to_le_bytes());self.tile_identity_buffer.extend(part);}
        self.tile_identity_buffer.len()
    }
    pub fn layout_identity_byte(&self,index:usize)->u32 {self.tile_identity_buffer.get(index).copied().unwrap_or(0) as u32}
    pub fn layout_floating_field(&self,id:u32,field:u32)->u32 {
        self.auto_tile_restore.get(&id).map_or(0,|(x,y,w,h)|match field {0=>*x as i32 as u32,1=>*y as i32 as u32,2=>u32::from(*w),3=>u32::from(*h),_=>0,
            })
    }
    pub fn restore_layout_floating(&mut self,id:u32,x:i32,y:i32,width:u32,height:u32,
    )->bool {
        if !self.managed_frames.contains_key(&id)||width==0||height==0 {return false;}
        let (ax,ay,aw,ah)=self.wm_workarea_rect();
        let (left,right,top,bottom)=self.managed_frame_insets(ResourceId(id));
        let width=width.min(u32::from(aw.saturating_sub(left+right).max(1))) as u16;
        let height=height.min(u32::from(ah.saturating_sub(top+bottom).max(1))) as u16;
        let x=x.clamp(i32::from(ax),i32::from(ax)+i32::from(aw.saturating_sub(width+left+right)),
        );
        let y=y.clamp(i32::from(ay),i32::from(ay)+i32::from(ah.saturating_sub(height+top+bottom)),
        );
        self.auto_tile_restore.insert(id,(x as i16,y as i16,width,height));true
    }
    pub fn begin_workspace_layout_load(&mut self) {self.tile_snapshot_buffer.clear();}
    pub fn push_workspace_layout_word(&mut self,word:u32)->bool {
        if self.tile_snapshot_buffer.len()>=4096 {return false;}
        self.tile_snapshot_buffer.push(word);true
    }
    pub fn restore_workspace_layout(&mut self,workspace:u32)->bool {
        let words=&self.tile_snapshot_buffer;
        if workspace>=self.workspace_count || words.len()<7 || words[0]!=1 || words[1]>256 {return false;}
        let count=words[1] as usize;
        if words.len()<2+count*2+5 {return false;}
        let mut members=Vec::new();
        for pair in words[2..2+count*2].chunks_exact(2) {
            let target=match pair[0] {
                0 if self.managed_frames.contains_key(&pair[1])&&self.window_workspaces.get(&pair[1])==Some(&workspace)=>{
                    WmTaskTarget::X11(ResourceId(pair[1]))}
                1 if self.native_tasks.iter().any(|task|u32::from(task.id)==pair[1]&&task.workspace==workspace)=>{
                    WmTaskTarget::Native(pair[1] as u16)}
                _=>return false,
            };
            if members.contains(&target) {return false;}
            members.push(target);
        }
        let Some(layout)=TileLayout::from_snapshot(&words[2+count*2..],|index|{
            members.get(index as usize).copied()}) else {return false;};
        self.tile_layouts.insert(workspace,layout);true
    }
    pub fn add_auto_tile_native(&mut self, id: u16) {
        if id != 0 && self.auto_tile_native.len() < 128 && !self.auto_tile_native.contains(&id) { self.auto_tile_native.push(id); }
    }
    pub fn set_auto_tile_native_minimum(&mut self, id: u16, width: u16, height: u16) {
        if !self.auto_tile_native.contains(&id) {return;}
        let target=WmTaskTarget::Native(id);
        self.auto_tile_minimums.retain(|(candidate,_,_)|*candidate!=target);
        self.auto_tile_minimums.push((target,width.saturating_add(8),height.saturating_add(8)));
    }
    // ICCCM 4.1.2.3: WM_NORMAL_HINTS is WM_SIZE_HINTS/32, with an old
    // 15-word or extended 18-word payload. Client sizes exclude WM chrome.
    pub(super) fn managed_client_size_limits(&self, client: ResourceId) -> (u16,u16,u16,u16) {
        let hints=self.windows.get(&client.0).and_then(|window|{
            window.properties.iter().find_map(|(atom,p)|
            {
                (self.atoms.name(*atom)==Some("WM_NORMAL_HINTS")&&self.atoms.name(p.r#type)==Some("WM_SIZE_HINTS")&&p.format==32&&p.data.len()>=60).then_some(&p.data)})
        });
        let Some(data)=hints else {return (0,0,u16::MAX,u16::MAX);};
        let flags=read_u32_le(&data[..4]);
        let dimension=|offset,fallback|{let value=read_u32_le(&data[offset..offset+4]) as i32;if value>0 {(value as u32).min(u32::from(u16::MAX)) as u16}else{fallback}};
        let (min_width,min_height)=if flags&16!=0 {(dimension(20,0),dimension(24,0))}
            else if flags&256!=0&&data.len()>=72 {(dimension(60,0),dimension(64,0))}else{(0,0)};
        let (max_width,max_height)=if flags&32!=0 {(dimension(28,u16::MAX),dimension(32,u16::MAX))}else{(u16::MAX,u16::MAX)};
        // Contradictory maxima cannot undercut the application's minimum.
        (min_width,min_height,max_width.max(min_width),max_height.max(min_height),
        )
    }
    pub(super) fn managed_tile_minimum(&self, client: ResourceId) -> (u16,u16) {
        let (width,height,_,_)=self.managed_client_size_limits(client);
        if width==0&&height==0 {return (0,0);}
        let (left,right,top,bottom)=self.managed_frame_insets(client);
        (width.saturating_add(left).saturating_add(right).saturating_add(8),
         height.saturating_add(top).saturating_add(bottom).saturating_add(8),
        )
    }
    pub fn auto_tile_rect(&self, index: usize, field: usize) -> u32 {
        self.auto_tile_rects.get(index).map_or(0, |r| [u32::from(r.0),r.1 as u32,r.2 as u32,u32::from(r.3),u32::from(r.4)][field.min(4)])
    }
    pub fn apply_auto_tile(&mut self) -> usize {
        if self.auto_tile_workspace>=self.workspace_count {return 0;}
        if self.auto_tile_enabled {self.tiled_workspaces.insert(self.auto_tile_workspace);}else{self.tiled_workspaces.remove(&self.auto_tile_workspace);}
        if !self.auto_tile_enabled {
            self.overview_placed_windows.retain(|id|self.window_workspaces.get(id).copied()!=Some(self.auto_tile_workspace));
            let restoring: Vec<_> = self.auto_tile_restore.keys().copied().filter(|id|
                {
                    self.window_workspaces.get(id).copied() == Some(self.auto_tile_workspace)
            }).collect();
            for id in restoring {
                if let Some((mut x,mut y,mut w,mut h)) = self.auto_tile_restore.remove(&id) {
                    if let Some(frame) = self.managed_frames.get(&id).copied() {
                        let client=ResourceId(id);
                        let (minimum_width,minimum_height,maximum_width,maximum_height)=self.managed_client_size_limits(client);
                        let (left,right,top,bottom)=self.managed_frame_insets(client);
                        if w<minimum_width||h<minimum_height||w>maximum_width||h>maximum_height {
                            let (ax,ay,aw,ah)=self.wm_workarea_rect();
                            w=w.max(minimum_width).min(maximum_width).min(aw.saturating_sub(left+right)).max(1);
                            h=h.max(minimum_height).min(maximum_height).min(ah.saturating_sub(top+bottom)).max(1);
                            x=i32::from(x).clamp(i32::from(ax),i32::from(ax)+i32::from(aw.saturating_sub(w.saturating_add(left+right))),
                            ) as i16;
                            y=i32::from(y).clamp(i32::from(ay),i32::from(ay)+i32::from(ah.saturating_sub(h.saturating_add(top+bottom))),
                            ) as i16;
                        }
                        self.apply_managed_geometry(ResourceId(id),frame,x,y,w,h);
                    }
                }
            }
            return 0;
        }
        let scrolling=self.tile_layouts.get(&self.auto_tile_workspace).is_some_and(|layout|layout.scrolling);
        let clients: Vec<_> = self.managed_frames.keys().copied()
            .filter(|id|self.window_workspaces.get(id)==Some(&self.auto_tile_workspace)&&!self.parked_managed_windows.contains(id)
                &&self.managed_transient_owner_relation(ResourceId(*id)).is_none())
            .map(|id|WmTaskTarget::X11(ResourceId(id))).filter_map(|target| match target {
            WmTaskTarget::X11(id) if (scrolling||!self.overview_placed_windows.contains(&id.0)) && !self.minimized_windows.contains(&id.0) && !self.non_resizable_managed_windows.contains(&id.0) && (scrolling||!self.restore_geometries.contains_key(&id.0)) && !self.managed_client_is_modal(id) => {
                    Some(id)}
                _=>None
        ,
            }).collect();
        self.auto_tile_native.sort_unstable();
        let members: Vec<_> = clients.iter().copied().map(WmTaskTarget::X11)
            .chain(self.auto_tile_native.iter().copied().map(WmTaskTarget::Native),
            ).collect();
        let (x,y,width,height) = self.wm_workarea_rect();
        let area=TileRect{x:i32::from(x),y:i32::from(y),width,height,
        };
        let mut minimums=self.auto_tile_minimums.clone();
        minimums.extend(clients.iter().map(|client|{let (width,height)=self.managed_tile_minimum(*client);(WmTaskTarget::X11(*client),width,height)}));
        // Output rectangles must include the panels: wm_output_workarea below
        // subtracts their reservations exactly once, including in browser mode.
        let physical=if self.wm_outputs.is_empty(){vec![(0,0,self.config.width_px,self.config.height_px)]}else{self.wm_outputs.clone()};
        let areas:Vec<_>=physical.iter().map(|&o|{let (x,y,width,height)=self.wm_output_workarea(o,false);TileRect{x:i32::from(x),y:i32::from(y),width,height}}).collect();
        let assignments:Vec<_>=members.iter().map(|&id|{
            let point=match id {
                WmTaskTarget::Native(id)=>self.auto_tile_positions.iter().find(|(w,_,_)|*w==id).map(|(_,x,y)|(*x,*y)),
                WmTaskTarget::X11(id)=>self.auto_tile_restore.get(&id.0).map(|&(x,y,w,h)|(i32::from(x)+i32::from(w)/2,i32::from(y)+i32::from(h)/2)).or_else(||self.managed_frames.get(&id.0).and_then(|f|self.windows.get(&f.0)).map(|w|(i32::from(w.x)+i32::from(w.width)/2,i32::from(w.y)+i32::from(w.height)/2))),
                _=>None,
            };
            let index=point.and_then(|(x,y)|physical.iter().position(|&(ox,oy,w,h)|x>=i32::from(ox)&&x<i32::from(ox)+i32::from(w)&&y>=i32::from(oy)&&y<i32::from(oy)+i32::from(h))).unwrap_or(0);
            (id,index as u32)
        }).collect();
        let fullscreens:Vec<_>=assignments.iter().filter(|(id,_)|self.tile_fullscreen(*id)).map(|&(id,output)|(id,physical[output as usize].2)).collect();
        let active=self.auto_tile_focus.or_else(||self.active_wm_task_target());
        let layout=self.tile_layouts.entry(self.auto_tile_workspace).or_default();
        if layout.scrolling{layout.strip.configure(&areas,&assignments);layout.strip.set_fullscreens(&fullscreens);}
        layout.set_minimums(minimums);
        layout.sync(&members,area);
        if layout.scrolling {layout.strip.focus(active,area);}
        let rectangles=layout.rectangles(area);
        for (target,mut cell) in rectangles {
            let fullscreen=self.tile_fullscreen(target)&&scrolling;
            if fullscreen {
                if let (Some(view),Some(full))=(self.tile_layouts[&self.auto_tile_workspace].strip.clip(target),self.scrolling_clip(target)) {
                    cell=TileRect{x:full.x+cell.x-view.x,y:full.y,width:full.width,height:full.height};
                }
            }
            if let Some(clip)=self.scrolling_clip(target) {
                match target {WmTaskTarget::Native(id)=>self.auto_tile_clips.push((1,u32::from(id),clip)),WmTaskTarget::X11(id)=>{if let Some(frame)=self.managed_frames.get(&id.0){self.auto_tile_clips.push((0,frame.0,clip));}},_=>{}}
            }
            let gap=if fullscreen{0}else{4u16.min(cell.width/4).min(cell.height/4)};
            let rect=(cell.x.saturating_add(i32::from(gap)),cell.y.saturating_add(i32::from(gap)),cell.width.saturating_sub(gap*2).max(1),cell.height.saturating_sub(gap*2).max(1),
            );
            if let WmTaskTarget::X11(client) = target {
                let frame=self.managed_frames[&client.0];
                let (Some(c),Some(f))=(self.windows.get(&client.0),self.windows.get(&frame.0)) else {continue;};
                let old=(f.x,f.y,c.width,c.height);
                self.auto_tile_restore.entry(client.0).or_insert(old);
                let (left, right, top, bottom) = self.managed_frame_insets(client);
                let next=(rect.0.clamp(i32::from(i16::MIN),i32::from(i16::MAX)) as i16,rect.1.clamp(i32::from(i16::MIN),i32::from(i16::MAX)) as i16,rect.2.saturating_sub(left.saturating_add(right)).max(1),rect.3.saturating_sub(top.saturating_add(bottom)).max(1),
                );
                if old!=next {
                    if old.2==next.2 && old.3==next.3 {
                        // Panning is a scene translation, not a client resize.
                        // Keep retained pixels; never expose/repaint app contents.
                        if let Some(f)=self.windows.get_mut(&frame.0) {f.x=next.0;f.y=next.1;}
                        if let Some(c)=self.windows.get(&client.0).cloned() {
                            self.emit_managed_configure_notify(&c,Geometry{root:ROOT_WINDOW,
                                x:next.0.saturating_add(left as i16),y:next.1.saturating_add(top as i16),
                                width:c.width,height:c.height,border_width:0,depth:c.depth},true);
                        }
                        self.scene_upsert(frame);
                    } else {self.apply_managed_geometry(client,frame,next.0,next.1,next.2,next.3);}
                }
            } else if let WmTaskTarget::Native(id) = target { self.auto_tile_rects.push((id,rect.0,rect.1,rect.2,rect.3)); }
        }
        self.auto_tile_rects.len()
    }

    pub(super) fn arrange_all_task_windows(&mut self, tile: bool) {
        let members = self.wm_raw_task_targets();
        self.arrange_x11_task_stack(&members, tile);
        if members
            .iter()
            .any(|target| matches!(target, WmTaskTarget::Native(_)))
        {
            self.wm_actions.push_back(if tile {
                WM_ACTION_STACK_TILE_PREFIX
            } else {
                WM_ACTION_STACK_CASCADE_PREFIX
            });
        }
    }

    pub(super) fn arrange_x11_task_stack(&mut self, members: &[WmTaskTarget], tile: bool) {
        let clients = members
            .iter()
            .filter_map(|target| match target {
                WmTaskTarget::X11(client) => Some(*client),
                _ => None,
            })
            .filter(|client| !self.non_resizable_managed_windows.contains(&client.0))
            .collect::<Vec<_>>();
        if clients.is_empty() {
            return;
        }
        let (work_x, work_y, work_width, work_height) = self.wm_workarea_rect();
        if tile {
            let mut columns = 1usize;
            while columns.saturating_mul(columns) < clients.len() {
                columns += 1;
            }
            let rows = clients.len().div_ceil(columns);
            let cell_width = usize::from(work_width) / columns;
            let cell_height = usize::from(work_height) / rows.max(1);
            for (index, client) in clients.into_iter().enumerate() {
                let column = index % columns;
                let row = index / columns;
                self.minimized_windows.remove(&client.0);
                self.maximized_geometries.remove(&client.0);
                self.snapped_geometries.remove(&client.0);
                let frame = self.managed_frames[&client.0];
                let (left, right, top, bottom) = self.managed_frame_insets(client);
                self.apply_managed_geometry(
                    client,
                    frame,
                    i16::try_from(usize::from(work_x) + column * cell_width).unwrap_or(i16::MAX),
                    i16::try_from(usize::from(work_y) + row * cell_height).unwrap_or(i16::MAX),
                    u16::try_from(cell_width)
                        .unwrap_or(u16::MAX)
                        .saturating_sub(left.saturating_add(right)),
                    u16::try_from(cell_height)
                        .unwrap_or(u16::MAX)
                        .saturating_sub(top.saturating_add(bottom)),
                );
            }
        } else {
            let width = work_width.saturating_mul(3) / 4;
            let height = work_height.saturating_mul(3) / 4;
            for (index, client) in clients.into_iter().enumerate() {
                let offset = u16::try_from(index % 8).unwrap_or(0) * 28;
                self.minimized_windows.remove(&client.0);
                self.maximized_geometries.remove(&client.0);
                self.snapped_geometries.remove(&client.0);
                let frame = self.managed_frames[&client.0];
                let (left, right, top, bottom) = self.managed_frame_insets(client);
                self.apply_managed_geometry(
                    client,
                    frame,
                    i16::try_from(work_x.saturating_add(offset)).unwrap_or(i16::MAX),
                    i16::try_from(work_y.saturating_add(offset)).unwrap_or(i16::MAX),
                    width.saturating_sub(left.saturating_add(right)),
                    height.saturating_sub(top.saturating_add(bottom)),
                );
            }
        }
        self.rebuild_scene();
    }
}

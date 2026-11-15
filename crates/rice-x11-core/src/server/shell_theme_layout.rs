//! Shared theme menu rectangles for native painting and pointer input.
use super::*;

impl DisplayServer {
    /// Shared browser/native managed-frame geometry. A zero title restores defaults.
    pub fn set_window_chrome(&mut self, values:[u32;15])->bool {
        let next=if values[4]==0 {None} else {
            let ranges=[(0,32),(0,32),(0,32),(0,32),(20,72),(12,96),(12,64),(0,16),(0,32),(0,1),(0,1),(0,1),(0,96),(1,4)];
            if values[..14].iter().zip(ranges).any(|(&v,(min,max))|!(min..=max).contains(&v)) {return false;}
            let count=values[13];let mut seen=0;
            for i in 0..count {let action=(values[14]>>(i*4))&15;if !(1..=4).contains(&action)||seen&(1<<action)!=0{return false;}seen|=1<<action;}
            if values[14]>>(count*4)!=0{return false;}
            Some(values)
        };
        if self.wm_window_chrome==next{return true;}
        self.wm_window_chrome=next;
        let clients:Vec<_>=self.managed_frames.keys().copied().map(ResourceId).collect();
        for client in clients {
            if self.maximized_geometries.contains_key(&client.0) {
                let frame=self.managed_frames[&client.0];
                if let Some(state)=self.windows.get(&frame.0).cloned(){
                    let (l,r,t,b)=self.managed_frame_insets(client);
                    self.publish_managed_frame_extents(client);
                    self.apply_managed_geometry(client,frame,state.x,state.y,state.width.saturating_sub(l+r).max(1),state.height.saturating_sub(t+b).max(1));
                }
            } else {self.refresh_managed_decorations(client);}
        }
        self.repaint_wm_chrome();self.rebuild_scene();true
    }

    pub(super) fn custom_frame_resize_edges(&self,width:u16,height:u16,x:i32,y:i32)->u8 {
        let Some(v)=self.wm_window_chrome else{return 0;};
        let border=WM_RESIZE_HIT_WIDTH.max(v[1].max(v[2]).max(v[3])as i32);
        let corner=border.max((v[12]as i32).min(i32::from(width)/2).min(i32::from(height)/2));
        let w=i32::from(width);let h=i32::from(height);
        let horizontal=y<border||y>=h-border;let vertical=x<border||x>=w-border;
        let mut edges=0;
        if x<border||horizontal&&x<corner {edges|=WM_RESIZE_LEFT;}else if x>=w-border||horizontal&&x>=w-corner {edges|=WM_RESIZE_RIGHT;}
        if y<border||vertical&&y<corner {edges|=WM_RESIZE_TOP;}else if y>=h-border||vertical&&y>=h-corner {edges|=WM_RESIZE_BOTTOM;}
        edges
    }

    pub(super) fn custom_frame_control_at(&self,width:u16,x:i32,y:i32)->Option<u8> {
        let v=self.wm_window_chrome?;
        let h=v[6].min(v[4].saturating_sub(v[0]));
        let top=v[0]+(v[4].saturating_sub(v[0]+h))/2;
        if y<(top as i32)||y>=((top+h)as i32){return None;}
        let count=v[13];let pitch=v[5]+v[7];
        for i in 0..count {
            let left=if v[9]!=0&&i==0 {v[3]+v[8]} else if v[9]==0&&v[10]!=0 {v[3]+4+v[8]+pitch*i} else {
                u32::from(width).saturating_sub(v[1]+v[8]+pitch*(count-if v[9]!=0{1}else{0}))+pitch*(i-if v[9]!=0{1}else{0})
            };
            if x>=left as i32&&x<(left+v[5])as i32 {return Some(((v[14]>>(i*4))&15)as u8-1);}
        }
        None
    }

    /// Zero dimensions restore the selected profile's geometry.
    pub fn set_theme_shell_layout(&mut self, values: [usize; 7]) -> bool {
        let ranges = [(36,320),(160,600),(120,400),(0,64),(0,96),(0,64),(1,2)];
        if values.iter().zip(ranges).enumerate().any(|(i,(&v,(min,max)))|
            !(v==0 && i<6) && !(min..=max).contains(&v)) {return false;}
        if self.wm_theme_shell_layout==values {return true;}
        self.wm_theme_shell_layout=values;
        self.wm_menu_hover_row=None;
        self.refresh_taskbar_geometry();
        self.relayout_wm_menu();
        self.repaint_wm_chrome();
        self.rebuild_scene();
        true
    }

    pub(super) fn wm_theme_menu_cascade_panel(&self,column:usize)->bool {
        self.wm_menu_is_start_cascade() || (matches!(self.wm_menu_level,WmMenuLevel::ApplicationContext(_)|WmMenuLevel::MenuDirectoryContext)
            && column<self.wm_application_context_columns.len()
            && matches!(self.wm_application_context_parent_level,WmMenuLevel::Root|WmMenuLevel::Categories|WmMenuLevel::Applications(_)|WmMenuLevel::MenuFolders|WmMenuLevel::MenuFolder(_)|WmMenuLevel::Themes))
    }

    pub(super) fn wm_theme_menu_root(&self,column:usize)->bool {
        column==0 && self.wm_theme_menu_cascade_panel(column) && !self.wm_menu_applications_only
    }

    pub(super) fn wm_theme_menu_split(&self,column:usize,rows:&[WmMenuRow])->usize {
        if self.wm_theme_menu_root(column) && self.wm_theme_shell_layout[6]==2 && !self.wm_compact_shell() {
            rows.len().div_ceil(2)
        } else {rows.len()}
    }

    pub(super) fn wm_theme_menu_insets(&self,column:usize)->(usize,usize,usize) {
        let root=self.wm_theme_menu_root(column);
        (if self.wm_theme_menu_cascade_panel(column){self.wm_theme_shell_layout[3]}else{0},
            if root {self.wm_theme_shell_layout[4]} else {0},
            if root {self.wm_theme_shell_layout[5]} else {0})
    }

    pub(super) fn wm_theme_menu_panel_height(&self,column:usize,rows:&[WmMenuRow])->usize {
        let (_,header,footer)=self.wm_theme_menu_insets(column);
        let split=self.wm_theme_menu_split(column,rows);
        let row_height=self.wm_menu_base_row_height();
        header+footer+wm_menu_rows_height_with(&rows[..split],row_height)
            .max(wm_menu_rows_height_with(&rows[split..],row_height))
    }

    /// Rectangles relative to a panel, excluding its decorative rail and bands.
    pub(super) fn wm_theme_menu_row_rects(&self,column:usize,rows:&[WmMenuRow],width:usize)->Vec<(usize,usize,usize,usize)> {
        let (rail,header,_)=self.wm_theme_menu_insets(column);
        let split=self.wm_theme_menu_split(column,rows);
        let columns=if split<rows.len(){2}else{1};
        let cell_width=width.saturating_sub(rail)/columns;
        let mut y=header;
        rows.iter().enumerate().map(|(index,row)|{
            if index==split {y=header;}
            let right=index>=split;
            let x=rail+if right{cell_width}else{0};
            let height=wm_menu_row_height_with(row,self.wm_menu_base_row_height());
            let rect=(x,y,if right{width.saturating_sub(x)}else{cell_width},height);
            y=y.saturating_add(height);
            rect
        }).collect()
    }
}

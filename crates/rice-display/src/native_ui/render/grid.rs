//! Native virtual grid drawing. One widget produces bounded cell commands.
use super::super::{DrawCommand, NativeUiError, TextAlign, TextRole};
use super::helpers::push_command;
use rice_ui::grid_frame::{GridCell, GridFrame, GridTrack, column_label};
use rice_ui::{Color, Insets, Point, Rect, Size, Theme, Widget};

fn local_rect(origin: Point, x: i64, y: i64, width: u32, height: u32) -> Rect {
    Rect {
        origin: Point {
            x: origin
                .x
                .saturating_add(i32::try_from(x).unwrap_or(if x < 0 {
                    i32::MIN
                } else {
                    i32::MAX
                })),
            y: origin
                .y
                .saturating_add(i32::try_from(y).unwrap_or(if y < 0 {
                    i32::MIN
                } else {
                    i32::MAX
                })),
        },
        size: Size { width, height },
    }
}


fn text_insets(zoom:u16)->Insets {let scale=|n:u32|(n*u32::from(zoom)+50)/100;Insets{left:scale(4),right:scale(4),top:scale(1),bottom:scale(1)}}
pub(super) fn paint_color(value: &str) -> Color { let rgb=u32::from_str_radix(&value[1..],16).unwrap_or(0);Color::opaque((rgb>>16)as u8,(rgb>>8)as u8,rgb as u8) }
pub(super) fn automatic_ink(background:Color, preferred:Color)->Color {
    let luminance=|color:Color| {let [r,g,b,_]=color.channels();let linear=|v:u8|{let c=f64::from(v)/255.0;if c<=0.04045{c/12.92}else{((c+0.055)/1.055).powf(2.4)}};0.2126*linear(r)+0.7152*linear(g)+0.0722*linear(b)};
    let bg=luminance(background);let fg=luminance(preferred);
    if (bg.max(fg)+0.05)/(bg.min(fg)+0.05)>=4.5 {preferred}
    else if (bg+0.05)/0.05 >= 1.05/(bg+0.05) {Color::opaque(0,0,0)} else {Color::opaque(255,255,255)}
}
fn selected_fill(base: Color, accent: Color)->Color { let b=base.channels();let a=accent.channels();Color::opaque(((u16::from(b[0])*3+u16::from(a[0]))/4)as u8,((u16::from(b[1])*3+u16::from(a[1]))/4)as u8,((u16::from(b[2])*3+u16::from(a[2]))/4)as u8) }
fn border(commands:&mut Vec<DrawCommand>,bounds:Rect,edge:usize,style:&rice_ui::grid_frame::GridBorder,zoom:u16)->Result<(),NativeUiError>{
    let width=match style.style.as_str(){"medium"|"mediumDashed"|"mediumDashDot"|"mediumDashDotDot"=>2,"thick"=>3,_=>1};
    let width=((width*u32::from(zoom)+50)/100).max(1);
    let horizontal=edge==0||edge==2;let mut line=bounds;
    if horizontal {line.size.height=width;if edge==2{line.origin.y=line.origin.y.saturating_add_unsigned(bounds.size.height.saturating_sub(width));}}
    else {line.size.width=width;if edge==1{line.origin.x=line.origin.x.saturating_add_unsigned(bounds.size.width.saturating_sub(width));}}
    let color=paint_color(&style.color);
    let pattern=match style.style.as_str(){"dotted"=>1,"dashed"|"mediumDashed"=>2,"dashDot"|"mediumDashDot"=>3,"dashDotDot"|"mediumDashDotDot"=>4,"slantDashDot"=>5,"hair"=>6,_=>0};
    push_command(commands,DrawCommand::StrokeLine{rect:line,color,width:width as u16,pattern})?;
    if style.style=="double" {if horizontal {line.origin.y=line.origin.y.saturating_add(if edge==0{((2*u32::from(zoom)+50)/100).max(1)as i32}else{-(((2*u32::from(zoom)+50)/100).max(1)as i32)});}else{line.origin.x=line.origin.x.saturating_add(if edge==3{((2*u32::from(zoom)+50)/100).max(1)as i32}else{-(((2*u32::from(zoom)+50)/100).max(1)as i32)});}push_command(commands,DrawCommand::StrokeLine{rect:line,color,width:width as u16,pattern:0})?;}
    Ok(())
}

fn text(
    commands: &mut Vec<DrawCommand>,
    rect: Rect,
    value: String,
    color: Color,
    align: TextAlign,
    theme: &Theme,
    zoom: u16,
) -> Result<(), NativeUiError> {
    if zoom!=100 { return push_command(commands,DrawCommand::GridText{zoom_percent:zoom,rect:rect.inset(text_insets(zoom)),text:value,color,align,font:None,size_px:theme.typography.body_px,vertical:1,wrap:false,strike:false}); }
    push_command(
        commands,
        DrawCommand::Text {
            rect: rect.inset(Insets {
                left: 4,
                right: 4,
                top: 1,
                bottom: 1,
            }),
            text: value,
            color,
            size_px: theme.typography.body_px,
            align,
            role: TextRole::Label,
            caret: false,
            caret_offset: None,
            brand: false,
            gradient: false,
        },
    )
}

fn header(
    commands: &mut Vec<DrawCommand>,
    clip: Rect,
    bounds: Rect,
    value: String,
    reserve_right: u32,
    theme: &Theme,
    zoom: u16,
) -> Result<(), NativeUiError> {
    push_command(commands, DrawCommand::PushClip { rect: clip })?;
    push_command(
        commands,
        DrawCommand::Fill {
            rect: bounds,
            color: theme.palette.surface_raised,
            radius: 0,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect: bounds,
            color: theme.palette.border,
            width: 1,
            radius: 0,
        },
    )?;
    text(
        commands,
        Rect {
            size: Size {
                width: bounds.size.width.saturating_sub(reserve_right),
                ..bounds.size
            },
            ..bounds
        },
        value,
        theme.palette.text_muted,
        TextAlign::Center,
        theme,
        zoom,
    )?;
    push_command(commands, DrawCommand::PopClip)
}

// Merge only physically adjacent visible pieces, so frozen panes and filtered
// rows never produce a box across unrelated tracks. Work is bounded by the frame.
fn reference_segments(tracks: &[GridTrack], first: u32, last: u32) -> Vec<(u32, u32)> {
    let mut result: Vec<(u32, u32)> = Vec::new();
    for track in tracks.iter().filter(|t| (first..=last).contains(&t.index)) {
        if let Some(previous) = result.last_mut() {
            if previous.1 == track.clip_start { previous.1 = track.clip_end; continue; }
        }
        result.push((track.clip_start, track.clip_end));
    }
    result
}

fn reference_edge(track: &GridTrack, trailing: bool) -> Option<(u32, u32)> {
    let edge = if trailing { track.start + i64::from(track.extent).saturating_sub(2) } else { track.start };
    let start = edge.max(i64::from(track.clip_start));
    let end = (edge + 2).min(i64::from(track.clip_end));
    (end > start).then_some((start as u32, (end - start).max(0) as u32))
}

fn references(frame: &GridFrame, body: Point, theme: &Theme, commands: &mut Vec<DrawCommand>) -> Result<(), NativeUiError> {
    for reference in &frame.references {
        let color = if frame.canvas.as_deref()==Some("document") {theme.formula_reference_ink_on(reference.color,Color::opaque(255,255,255))} else {theme.formula_reference_ink(reference.color)};
        let rows = reference_segments(&frame.rows, reference.first_row, reference.last_row);
        let columns = reference_segments(&frame.columns, reference.first_column, reference.last_column);
        for (index, trailing) in [(reference.first_row, false), (reference.last_row, true)] {
            if let Some((y, height)) = frame.rows.iter().find(|t| t.index == index).and_then(|t| reference_edge(t, trailing)) {
                for &(start, end) in &columns {
                    push_command(commands, DrawCommand::Fill { rect: local_rect(body, i64::from(start), i64::from(y), end-start, height), color, radius: 0 })?;
                }
            }
        }
        for (index, trailing) in [(reference.first_column, false), (reference.last_column, true)] {
            if let Some((x, width)) = frame.columns.iter().find(|t| t.index == index).and_then(|t| reference_edge(t, trailing)) {
                for &(start, end) in &rows {
                    push_command(commands, DrawCommand::Fill { rect: local_rect(body, i64::from(x), i64::from(start), width, end-start), color, radius: 0 })?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn render(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if !widget.children().is_empty() {
        return Err(NativeUiError::InvalidPresentation("grid must be a leaf"));
    }
    let frame = GridFrame::parse(widget.text.as_deref().unwrap_or(""))
        .map_err(NativeUiError::InvalidPresentation)?;
    let document = frame.canvas.as_deref() == Some("document");
    let paper = Color::opaque(255,255,255);
    let ink = Color::opaque(0,0,0);
    let gridline = if document {Color::opaque(218,220,224)} else {theme.palette.border};
    // The parent render path also applies the widget/ancestor clip. A resized
    // widget may temporarily contain an older frame; never stretch cell IDs.
    let viewport = Rect {
        origin: rect.origin,
        size: Size {
            width: rect.size.width.min(frame.width),
            height: rect.size.height.min(frame.height),
        },
    };
    push_command(commands, DrawCommand::PushClip { rect: viewport })?;
    push_command(
        commands,
        DrawCommand::Fill {
            rect,
            color: if document {paper} else {theme.palette.surface},
            radius: 0,
        },
    )?;
    let body = Point {
        x: rect.origin.x.saturating_add_unsigned(frame.row_header),
        y: rect.origin.y.saturating_add_unsigned(frame.column_header),
    };
    let mut values = frame.cells.iter().peekable();
    for (row_slot, row) in frame.rows.iter().enumerate() {
        for (column_slot, column) in frame.columns.iter().enumerate() {
            let mut clip = local_rect(
                body,
                i64::from(column.clip_start),
                i64::from(row.clip_start),
                column.clip_end - column.clip_start,
                row.clip_end - row.clip_start,
            );
            let mut bounds = local_rect(body, column.start, row.start, column.extent, row.extent);
            let value = if values.peek().is_some_and(|cell| {
                usize::from(cell.0) == row_slot && usize::from(cell.1) == column_slot
            }) {
                values.next()
            } else {
                None
            };
            let merged=frame.merges.iter().find(|m|m.contains(row.index,column.index));
            if let Some(m)=merged {
                // Consume any covered slot above, but draw the logical cell once.
                if m.row_slot!=row_slot as u32||m.column_slot!=column_slot as u32{continue;}
                let last_row=frame.rows.iter().rev().find(|t|(m.first_row..=m.last_row).contains(&t.index)).unwrap();
                let last_column=frame.columns.iter().rev().find(|t|(m.first_column..=m.last_column).contains(&t.index)).unwrap();
                clip=local_rect(body,i64::from(column.clip_start),i64::from(row.clip_start),last_column.clip_end-column.clip_start,last_row.clip_end-row.clip_start);
                bounds=local_rect(body,m.x,m.y,m.width,m.height);
            }
            let selected = frame
                .selection
                .as_ref()
                .is_some_and(|s| s.contains(row_slot as u32, column_slot as u32));
            let style = value.map_or(&frame.styles[0], |cell| &frame.styles[usize::from(cell.3)]);
            let paint=style.paint.as_ref();
            let explicit_fill=paint.and_then(|p|p.background.as_deref()).map(paint_color);
            let fill = if document {
                let base=explicit_fill.unwrap_or(paper);
                if selected {selected_fill(base,theme.palette.selection)} else {base}
            } else if let Some(fill)=explicit_fill { if selected {selected_fill(fill,theme.palette.selection)}else{fill} } else if selected {
                theme.palette.selection
            } else if style.role == 1 {
                theme.palette.surface_raised
            } else {
                theme.palette.surface
            };
            push_command(commands, DrawCommand::PushClip { rect: clip })?;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: bounds,
                    color: fill,
                    radius: 0,
                },
            )?;
            push_command(
                commands,
                DrawCommand::Border {
                    rect: bounds,
                    color: gridline,
                    width: 1,
                    radius: 0,
                },
            )?;
            if let Some(GridCell(_, _, string, _)) = value {
                let text_bounds = if frame.filter_header_row == Some(row.index)
                    && frame.filter_button(column.index).is_some() {
                    Rect { size: Size { width: bounds.size.width.saturating_sub(18), ..bounds.size }, ..bounds }
                } else { bounds };
                let color = if let Some(color)=paint.and_then(|p|p.foreground.as_deref()) {paint_color(color)} else if document {
                    automatic_ink(fill,ink)
                } else if selected {
                    theme.palette.selection_text
                } else {
                    match style.role {
                        2 => theme.palette.danger,
                        3 => theme.palette.text_muted,
                        _ => theme.palette.text,
                    }
                };
                let color=if paint.is_some_and(|p|p.foreground.is_none()&&p.background.is_some()){automatic_ink(fill,color)}else{color};
                let align = match style.alignment {
                    1 => TextAlign::Center,
                    2 => TextAlign::End,
                    _ => TextAlign::Start,
                };
                let filter_header=frame.filter_header_row==Some(row.index)&&frame.filter_columns.iter().any(|entry|entry.column==column.index);
                let mut font=style.font.clone();
                if filter_header {font.get_or_insert(rice_ui::grid_frame::GridFont{family:"RiceWM Sans".into(),size_pt:rice_ui::grid_frame::GridFontSize::new(f64::from(((u32::from(theme.typography.body_px)*3+2)/4).clamp(1,409))).expect("bounded theme header font"),weight:700,italic:false,underline:false}).weight=700;}
                if document || paint.is_some() {
                    let vertical=match paint.and_then(|paint|paint.vertical.as_deref()) {
                        Some("top")=>0, Some("center")=>1, Some("bottom")=>2,
                        _=>if document {2}else{1},
                    };
                    push_command(commands,DrawCommand::GridText{zoom_percent:frame.zoom_percent.unwrap_or(100),rect:text_bounds.inset(text_insets(frame.zoom_percent.unwrap_or(100))),text:frame.strings[usize::from(*string)].clone(),color,align,font,size_px:theme.typography.body_px,vertical,wrap:paint.and_then(|paint|paint.wrap).unwrap_or(false),strike:paint.and_then(|paint|paint.strike).unwrap_or(false)})?;
                } else if let Some(font) = &font {
                    push_command(commands, DrawCommand::StyledText {
                        zoom_percent:frame.zoom_percent.unwrap_or(100),
                        rect: text_bounds.inset(text_insets(frame.zoom_percent.unwrap_or(100))),
                        text: frame.strings[usize::from(*string)].clone(),
                        color, align, font:font.clone(),
                    })?;
                } else {
                text(
                    commands,
                    text_bounds,
                    frame.strings[usize::from(*string)].clone(),
                    color,
                    align,
                    theme,
                    frame.zoom_percent.unwrap_or(100),
                )?;
                }
            }
            if let Some(borders)=paint.and_then(|p|p.borders.as_ref()){for(edge,style)in[&borders.top,&borders.right,&borders.bottom,&borders.left].into_iter().enumerate(){if let Some(style)=style{border(commands,bounds,edge,style,frame.zoom_percent.unwrap_or(100))?;}}}
            if frame.active == Some(merged.map_or((row.index,column.index),|m|(m.first_row,m.first_column))) {
                push_command(
                    commands,
                    DrawCommand::Border {
                        rect: bounds,
                        color: theme.palette.focus,
                        width: 2,
                        radius: 0,
                    },
                )?;
            }
            push_command(commands, DrawCommand::PopClip)?;
        }
    }
    references(&frame, body, theme, commands)?;
    if let Some((handle, _)) = frame.fill_handle() {
        let handle_rect = local_rect(rect.origin, i64::from(handle.origin.x), i64::from(handle.origin.y), handle.size.width, handle.size.height);
        push_command(commands, DrawCommand::Fill {
            rect: handle_rect,
            color: theme.palette.focus, radius: 0,
        })?;
        // Focus and selection often share an accent. A contrasting themed
        // edge keeps the small drag affordance visible inside that selection.
        push_command(commands, DrawCommand::Border {
            rect: handle_rect, color: theme.palette.selection_text, width: 1, radius: 0,
        })?;
    }
    for row in &frame.rows {
        let clip = local_rect(
            rect.origin,
            0,
            i64::from(frame.column_header + row.clip_start),
            frame.row_header,
            row.clip_end - row.clip_start,
        );
        let bounds = local_rect(
            rect.origin,
            0,
            i64::from(frame.column_header) + row.start,
            frame.row_header,
            row.extent,
        );
        header(
            commands,
            clip,
            bounds,
            (u64::from(row.index) + 1).to_string(),
            0,
            theme,
            100,
        )?;
    }
    for column in &frame.columns {
        let clip = local_rect(
            rect.origin,
            i64::from(frame.row_header + column.clip_start),
            0,
            column.clip_end - column.clip_start,
            frame.column_header,
        );
        let bounds = local_rect(
            rect.origin,
            i64::from(frame.row_header) + column.start,
            0,
            column.extent,
            frame.column_header,
        );
        let button = frame.filter_button(column.index);
        header(
            commands,
            clip,
            bounds,
            column_label(column.index),
            if button.is_some() && frame.filter_header_row.is_none() { 18 } else { 0 },
            theme,
            100,
        )?;
        if let Some((local, active)) = button {
            let button = local_rect(
                rect.origin,
                i64::from(local.origin.x),
                i64::from(local.origin.y),
                local.size.width,
                local.size.height,
            );
            let button_clip = if frame.filter_header_row.is_some() { viewport } else { clip };
            push_command(commands, DrawCommand::PushClip { rect: button_clip })?;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: button,
                    color: if active {
                        theme.palette.selection
                    } else {
                        theme.palette.surface_raised
                    },
                    radius: 2,
                },
            )?;
            push_command(
                commands,
                DrawCommand::Border {
                    rect: button,
                    color: if active {
                        theme.palette.focus
                    } else {
                        theme.palette.border
                    },
                    width: 1,
                    radius: 2,
                },
            )?;
            // Three bounded raster bars form a downward triangle; independent
            // of font coverage, zoom shaping, or a DOM icon layer.
            let color = if active {
                theme.palette.selection_text
            } else {
                theme.palette.text_muted
            };
            let bars: &[(i64,u32)] = if active { &[(0,9),(1,7),(2,5),(3,3),(4,3),(5,3),(6,3)] } else { &[(0,7),(1,5),(2,3),(3,1)] };
            for &(offset, width) in bars {
                let arrow = local_rect(
                    button.origin,
                    if active {3+(9-width)/2} else {(4+offset)as u32} as i64,
                    i64::from(button.size.height / 2) - if active{3}else{2} + offset,
                    width,
                    1,
                );
                push_command(
                    commands,
                    DrawCommand::Fill {
                        rect: arrow,
                        color,
                        radius: 0,
                    },
                )?;
            }
            push_command(commands, DrawCommand::PopClip)?;
        }
    }
    let corner = local_rect(rect.origin, 0, 0, frame.row_header, frame.column_header);
    header(commands, corner, corner, String::new(), 0, theme,100)?;
    push_command(commands, DrawCommand::PopClip)
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    fn track(index: u32, start: i64, clip_start: u32, clip_end: u32, frozen: bool) -> GridTrack {
        GridTrack { index, start, extent: 20, clip_start, clip_end, frozen }
    }
    #[test]
    fn references_clip_real_edges_and_do_not_bridge_unrelated_tracks() {
        let tracks = [track(0,0,0,20,true),track(10,15,20,35,false),track(11,35,35,55,false),track(12,55,55,75,false)];
        assert_eq!(reference_segments(&tracks,10,11),vec![(20,55)]);
        assert_eq!(reference_segments(&tracks,1,9),vec![]);
        assert_eq!(reference_edge(&tracks[1],false),None,"clipped left edge stays offscreen");
        assert_eq!(reference_edge(&tracks[1],true),Some((33,2)));
        assert_eq!(reference_edge(&tracks[0],false),Some((0,2)));
        let separated=[track(1,0,0,20,false),track(9,20,20,40,false),track(2,40,40,60,false)];
        assert_eq!(reference_segments(&separated,1,2),vec![(0,20),(40,60)]);
    }
}

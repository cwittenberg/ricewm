import { cellKey } from './table-values.mjs';
function packed({color,opacity}) {
    const rgb = Number.parseInt(color.slice(1),16);
    return ((Math.round(opacity*255)<<24)|((rgb&255)<<16)|(rgb&0xff00)|(rgb>>>16))>>>0;
}
/** Source-backed table cells share their parent's native object transform. */
export function paintTable(builder, object, textFrames, selectedCell = null, editingCell = null) {
    for (const cell of object.table?.cells || []) {
        if (cell.covered) continue;
        const {x,y,width,height}=cell.bounds, rect=[x,y,width,height];
        if (cell.fill) builder.rect(rect,packed(cell.fill));
        const edges={top:[[x,y],[x+width,y]],right:[[x+width,y],[x+width,y+height]],bottom:[[x+width,y+height],[x,y+height]],left:[[x,y+height],[x,y]]};
        for(const [side,points] of Object.entries(edges)) {
            const line=cell.borders[side];
            if(line && line.width>0) builder.line(...points,line.width,packed(line));
        }
        const frame=textFrames.get(cellKey(object.id,cell.row,cell.column));
        if(frame && cellKey(object.id,cell.row,cell.column)!==editingCell) builder.pushClip(rect).append(frame.displayList).popClip();

    }
    paintTableSelection(builder, object, selectedCell);
}
/** Selection is editor chrome; callers paint it outside effect scopes. */
export function paintTableSelection(builder, object, selectedCell) {
    if (selectedCell?.object.id !== object.id) return;
    const cell=object.table.cells.find(cell=>!cell.covered && cell.row===selectedCell.row && cell.column===selectedCell.column);
    if (!cell) return;
    const {x,y,width:w,height:h}=cell.bounds, points=[[x,y],[x+w,y],[x+w,y+h],[x,y+h]];
    for (let i=0;i<4;i++) builder.line(points[i],points[(i+1)%4],1.5,0xffc47b25);
}
export function tableCellAt(object,point, borderMargin = 0) {
    if(!object?.table || !object.transform) return null;
    const [a,b,c,d,e,f]=object.transform, determinant=a*d-b*c;
    if(Math.abs(determinant)<1e-12) return null;
    const dx=point[0]-e,dy=point[1]-f, x=(d*dx-c*dy)/determinant, y=(-b*dx+a*dy)/determinant;
    // Leave a narrow outer edge available for moving the table as one object.
    const horizontal = borderMargin * Math.hypot(c, d) / Math.abs(determinant);
    const vertical = borderMargin * Math.hypot(a, b) / Math.abs(determinant);
    const bounds = object.bounds;
    if (borderMargin && (x < bounds.x + horizontal || x > bounds.x + bounds.width - horizontal ||
        y < bounds.y + vertical || y > bounds.y + bounds.height - vertical)) return null;
    return object.table.cells?.find(cell=>!cell.covered && x>=cell.bounds.x && y>=cell.bounds.y && x<cell.bounds.x+cell.bounds.width && y<cell.bounds.y+cell.bounds.height) || null;
}

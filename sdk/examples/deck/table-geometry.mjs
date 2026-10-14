/** Derived table hit/preview geometry; persisted tracks remain engine-owned. */
export function tableLocalPoint(object, point) {
    const m = object?.transform;
    if (!m || !point?.every(Number.isFinite)) return null;
    const [a,b,c,d,e,f] = m, determinant = a*d-b*c;
    if (Math.abs(determinant) < 1e-12) return null;
    const x=point[0]-e,y=point[1]-f;
    return [(d*x-c*y)/determinant,(-b*x+a*y)/determinant];
}

export function tableDividerAt(object, point, tolerance = 5) {
    if (!object?.table?.canEditStructure) return null;
    const local = tableLocalPoint(object, point);
    if (!local) return null;
    const {x=0,y=0,width,height} = object.bounds;
    const sx=Math.hypot(object.transform[0],object.transform[1]), sy=Math.hypot(object.transform[2],object.transform[3]);
    const tx=tolerance/Math.max(sx,1e-6),ty=tolerance/Math.max(sy,1e-6);
    if (local[0]<x-tx || local[0]>x+width+tx || local[1]<y-ty || local[1]>y+height+ty) return null;
    let edge=x;
    for(let index=0;index<object.table.columns-1;index++) {
        edge+=object.table.columnWidths[index];
        if(Math.abs(local[0]-edge)<=tx) return {axis:'column',index,point:local};
    }
    edge=y;
    for(let index=0;index<object.table.rows;index++) {
        edge+=object.table.rowHeights[index];
        if(Math.abs(local[1]-edge)<=ty) return {axis:'row',index,point:local};
    }
    return null;
}

export function resizedTableTracks(object, divider, point) {
    const local=tableLocalPoint(object,point);
    if(!local) return null;
    const columnWidths=[...object.table.columnWidths],rowHeights=[...object.table.rowHeights],i=divider.index;
    if(divider.axis==='column') {
        const total=columnWidths[i]+columnWidths[i+1],minimum=Math.min(24,total/2);
        const next=Math.max(minimum,Math.min(total-minimum,columnWidths[i]+local[0]-divider.point[0]));
        columnWidths[i]=next;columnWidths[i+1]=total-next;
    } else {
        rowHeights[i]=Math.max(20,Math.min(1e6-rowHeights.reduce((n,v,j)=>n+(j===i?0:v),0),rowHeights[i]+local[1]-divider.point[1]));
    }
    return {columnWidths,rowHeights};
}

export function tableWithTracks(object, tracks) {
    const starts=(values,start)=>{const result=[start];for(const value of values)result.push(result.at(-1)+value);return result;};
    const xs=starts(tracks.columnWidths,object.bounds.x||0),ys=starts(tracks.rowHeights,object.bounds.y||0);
    const width=xs.at(-1)-xs[0],height=ys.at(-1)-ys[0];
    const cells=object.table.cells.map(cell=>({...cell,bounds:{x:xs[cell.column],y:ys[cell.row],
        width:xs[cell.column+cell.columnSpan]-xs[cell.column],height:ys[cell.row+cell.rowSpan]-ys[cell.row]}}));
    return {...object,bounds:{...object.bounds,width,height},
        edit:object.edit?.box?{...object.edit,box:{...object.edit.box,width,height}}:object.edit,
        table:{...object.table,...tracks,cells}};
}

/** Native widgets cannot be painted under an arbitrary rotated affine map. */
export function inlineCellBox(active, size) {
    if(!active?.cell?.editable || !size) return null;
    const [a,b,c,d,e,f]=active.object.transform;
    if(Math.abs(b)>1e-6 || Math.abs(c)>1e-6 || a<=0 || d<=0) return null;
    const bounds=active.cell.bounds;
    return {x:(a*bounds.x+e)/size.width,y:(d*bounds.y+f)/size.height,
        width:a*bounds.width/size.width,height:d*bounds.height/size.height,scaleX:a,scaleY:d};
}

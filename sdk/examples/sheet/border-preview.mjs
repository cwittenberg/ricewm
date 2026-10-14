/** Small native preview of range-edge operations; never reads worksheet cells. */
export function borderPreviewFrame(range,format,operations) {
    const rows=Math.min(3,range.lastRow-range.firstRow+1),columns=Math.min(3,range.lastColumn-range.firstColumn+1),width=210,height=96;
    const tracks=(count,size)=>Array.from({length:count},(_,index)=>{const start=Math.floor(index*size/count),end=Math.floor((index+1)*size/count);return{index,start,extent:end-start,clip_start:start,clip_end:end,frozen:false};});
    const cells=[],styles=[];
    for(let row=0;row<rows;row++)for(let column=0;column<columns;column++){
        const borders={...(format.borders||{})};
        for(const operation of operations){
            const {edge,style,color}=operation;
            const touched=edge==='all'||edge==='none'?['top','right','bottom','left']:
                edge==='outline'?[...(row===0?['top']:[]),...(row===rows-1?['bottom']:[]),...(column===0?['left']:[]),...(column===columns-1?['right']:[])]:
                edge==='inside'?[...(row>0?['top']:[]),...(row<rows-1?['bottom']:[]),...(column>0?['left']:[]),...(column<columns-1?['right']:[])]:
                edge==='inside-horizontal'?[...(row>0?['top']:[]),...(row<rows-1?['bottom']:[])]:
                edge==='inside-vertical'?[...(column>0?['left']:[]),...(column<columns-1?['right']:[])]:
                edge==='top'&&row===0||edge==='bottom'&&row===rows-1||edge==='left'&&column===0||edge==='right'&&column===columns-1?[edge]:[];
            for(const side of touched){if(edge==='none'||style==='none')delete borders[side];else borders[side]={style,color};}
        }
        const paint={vertical:'center',...(format.fontColor?{foreground:format.fontColor}:{}),...(format.fillColor?{background:format.fillColor}:{}),...(Object.keys(borders).length?{borders}:{})};
        const index=styles.length;styles.push({alignment:1,role:0,paint});cells.push([row,column,0,index]);
    }
    return{version:1,revision:0,sheet:0,row_count:rows,column_count:columns,width,height,row_header:0,column_header:0,rows:tracks(rows,height),columns:tracks(columns,width),cells,strings:['Text'],styles,selection:null,active:null};
}
export function borderPreviewEdge(frame,x,y) {
    if(y<10)return'top';if(y>frame.height-10)return'bottom';if(x<10)return'left';if(x>frame.width-10)return'right';
    if(frame.rows.slice(1).some(track=>Math.abs(y-track.start)<8))return'inside-horizontal';
    if(frame.columns.slice(1).some(track=>Math.abs(x-track.start)<8))return'inside-vertical';return null;
}

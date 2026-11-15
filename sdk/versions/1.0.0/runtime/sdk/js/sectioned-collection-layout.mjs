/** Immutable geometry for a virtualized grouped list or wrapping collection.
 * Collapsed state belongs to the view. Stable caller IDs identify group headers;
 * item indices remain those of the complete document, including hidden groups.
 */
export class SectionedCollectionLayout {
    static controlFactory=Object.freeze({topics:['navigation','documents','visual'],constructor:'new SectionedCollectionLayout({itemCount,groups:[{id,start,count,collapsed:false}],columns:1,rowHeight:104,headerHeight:28})',methods:'rows, totalHeight, rowForItem(index), range(offset,height,{overscan:4}); immutable header/item rows with y/height and original item start/end indices. Groups cover items exactly once in source order, including empty headers. Use inert spacers outside range and keep captured inputs retained. Caller owns controls, collapse state and document history.'});
    constructor({itemCount,groups=[],columns=1,rowHeight=104,headerHeight=28}={}) {
        if(!Number.isInteger(itemCount)||itemCount<0||itemCount>65536||!Number.isInteger(columns)||columns<1||columns>256||![rowHeight,headerHeight].every(n=>Number.isFinite(n)&&n>0&&n<=100000)||!Array.isArray(groups)||groups.length>65536)throw TypeError('Invalid sectioned collection layout');
        const ids=new Set();let cursor=0;
        const checked=groups.map(group=>{
            if(!group||typeof group.id!=='string'||!group.id||group.id.length>256||ids.has(group.id)||group.start!==cursor||!Number.isInteger(group.count)||group.count<0||cursor+group.count>itemCount||group.collapsed!==undefined&&typeof group.collapsed!=='boolean')throw TypeError('Invalid collection group');
            ids.add(group.id);cursor+=group.count;return {...group};
        });
        if(checked.length&&cursor!==itemCount)throw TypeError('Incomplete collection groups');
        const rows=[],itemRows=Array(itemCount);let y=0;
        const append=row=>{const index=rows.length;rows.push(Object.freeze({...row,index,y}));y+=row.height;return index;};
        for(const group of checked.length?checked:[{id:null,start:0,count:itemCount}]) {
            const header=group.id===null?null:append({type:'header',group:group.id,start:group.start,end:group.start+group.count,height:headerHeight});
            if(group.collapsed) {for(let i=group.start;i<group.start+group.count;i++)itemRows[i]=header;continue;}
            for(let start=group.start;start<group.start+group.count;start+=columns) {
                const end=Math.min(start+columns,group.start+group.count),index=append({type:'items',group:group.id,start,end,height:rowHeight});
                for(let i=start;i<end;i++)itemRows[i]=index;
            }
        }
        this.rows=Object.freeze(rows);this.totalHeight=y;this.itemRows=Object.freeze(itemRows);Object.freeze(this);
    }
    rowForItem(index) {return Number.isInteger(index)&&index>=0&&index<this.itemRows.length?this.rows[this.itemRows[index]]:null;}
    range(offset,height,{overscan=0}={}) {
        if(!Number.isFinite(offset)||offset<0||!Number.isFinite(height)||height<0||!Number.isInteger(overscan)||overscan<0||overscan>256)throw TypeError('Invalid collection viewport');
        const locate=value=>{let low=0,high=this.rows.length;while(low<high){const mid=(low+high)>>>1,row=this.rows[mid];if(row.y+row.height<=value)low=mid+1;else high=mid;}return low;};
        const start=locate(offset);let end=start;
        if(height>0){let low=start,high=this.rows.length;while(low<high){const mid=(low+high)>>>1;if(this.rows[mid].y<offset+height)low=mid+1;else high=mid;}end=low;}
        return Object.freeze({start:Math.max(0,start-overscan),end:Math.min(this.rows.length,end+overscan)});
    }
}

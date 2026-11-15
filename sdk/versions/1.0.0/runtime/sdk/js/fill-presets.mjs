import {checkedResolvedPaint,transformedColor,colorWithOpacity} from './color-references.mjs';
/** Six reference variations; source identities remain editable theme paints. */
export function gradientPresets(base='#4472C4') {
    const paint=checkedResolvedPaint(typeof base==='string'?{color:base,opacity:1}:base);
    const light=transformedColor(paint,[{kind:'lumMod',value:40000},{kind:'lumOff',value:60000}]);
    const dark=transformedColor(paint,[{kind:'lumMod',value:50000}]);
    const stop=(offset,value)=>({offset,...value});
    const values=[
        {kind:'linear',angle:90,stops:[stop(0,paint),stop(1,transformedColor(paint,[{kind:'lumMod',value:60000}]))]},
        {kind:'linear',angle:90,stops:[stop(0,light),stop(1,paint)]},
        {kind:'linear',angle:0,stops:[stop(0,paint),stop(1,dark)]},
        {kind:'linear',angle:45,stops:[stop(0,light),stop(.5,paint),stop(1,dark)]},
        {kind:'radial',angle:0,focus:[.5,.5,.5,.5],stops:[stop(0,light),stop(1,dark)]},
        {kind:'linear',angle:90,stops:[stop(0,paint),stop(1,colorWithOpacity(paint,0))]},
    ];
    return values.map((gradient,index)=>({id:String(index),title:['Color to darker color','Light to color','Color to dark','Light, color and dark','Radial light to dark','Color to transparent'][index],gradient}));
}

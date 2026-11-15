/** Small Web Mercator geometry for native scene maps. No renderer or network client. */
const limit=85.0511287798066,mod=(n,d)=>((n%d)+d)%d;
const number=(v,min,max)=>{if(!Number.isFinite(v)||v<min||v>max)throw RangeError('Map coordinate out of range');return v;};
export function mapProject({latitude,longitude,zoom,tileSize=256}){
 number(latitude,-90,90);number(longitude,-180,180);number(zoom,0,22);number(tileSize,16,1024);
 const size=tileSize*2**zoom,lat=Math.max(-limit,Math.min(limit,latitude))*Math.PI/180;
 return{x:(longitude+180)/360*size,y:(1-Math.asinh(Math.tan(lat))/Math.PI)/2*size};
}
export function mapUnproject({x,y,zoom,tileSize=256}){
 number(x,-1e12,1e12);number(y,-1e12,1e12);number(zoom,0,22);number(tileSize,16,1024);const size=tileSize*2**zoom;
 return{longitude:mod(x/size*360,360)-180,latitude:Math.atan(Math.sinh(Math.PI*(1-2*Math.max(0,Math.min(size,y))/size)))*180/Math.PI};
}
export function visibleMapTiles({latitude,longitude,zoom,width,height,tileSize=256,maximumTiles=256}){
 if(!Number.isInteger(zoom)||!Number.isInteger(maximumTiles))throw TypeError('Map tile zoom and limit must be integers');
 number(width,1,8192);number(height,1,8192);number(maximumTiles,1,256);const c=mapProject({latitude,longitude,zoom,tileSize}),n=2**zoom,left=c.x-width/2,top=c.y-height/2;
 const x0=Math.floor(left/tileSize),x1=Math.ceil((left+width)/tileSize)-1,y0=Math.max(0,Math.floor(top/tileSize)),y1=Math.min(n-1,Math.ceil((top+height)/tileSize)-1);
 if((x1-x0+1)*(y1-y0+1)>maximumTiles)throw RangeError('Map viewport exceeds its tile budget');
 const tiles=[];for(let y=y0;y<=y1;y++)for(let x=x0;x<=x1;x++)tiles.push({z:zoom,x:mod(x,n),y,rect:{x:x*tileSize-left,y:y*tileSize-top,width:tileSize,height:tileSize}});return tiles;
}
export function panMap({latitude,longitude,zoom,dx,dy,tileSize=256}){
 number(dx,-1e7,1e7);number(dy,-1e7,1e7);const point=mapProject({latitude,longitude,zoom,tileSize});return mapUnproject({x:point.x-dx,y:point.y-dy,zoom,tileSize});
}
mapProject.controlFactory=Object.freeze({topics:['desktopUtilities','scene'],constructor:'mapProject({latitude,longitude,zoom,tileSize:256})',methods:'Returns world pixel {x,y}. Latitude -90..90 is clamped to Mercator limits; longitude -180..180, zoom 0..22.'});
mapUnproject.controlFactory=Object.freeze({topics:['desktopUtilities','scene'],constructor:'mapUnproject({x,y,zoom,tileSize:256})',methods:'Returns {latitude,longitude}, wrapping the date line and clamping polar coordinates.'});
visibleMapTiles.controlFactory=Object.freeze({topics:['desktopUtilities','scene'],constructor:'visibleMapTiles({latitude,longitude,zoom,width,height,tileSize:256,maximumTiles:256})',methods:'Integer zoom 0..22; returns [{z,x,y,rect:{x,y,width,height}}] in viewport coordinates, at most 256 tiles. Geometry only; fetch/decode and attribution remain app owned.'});
panMap.controlFactory=Object.freeze({topics:['desktopUtilities','scene'],constructor:'panMap({latitude,longitude,zoom,dx,dy,tileSize:256})',methods:'Converts native pointer displacement to a new {latitude,longitude}; dragging right moves the map right.'});

/** Chart paint follows host presentation metadata; these are not theme-edit grants. */
export const DEFAULT_CHART_THEME = Object.freeze({
    background:'#f7f8fa',panel:'#ffffff',foreground:'#20242d',muted:'#687180',
    border:'#dce1e9',accent:'#526de5',positive:'#22865e',negative:'#c84f60',
});
const keys = new Set(Object.keys(DEFAULT_CHART_THEME));
export function chartColor(value) {
    if (Number.isInteger(value) && value >= 0 && value <= 0xffffff) return '#'+value.toString(16).padStart(6,'0');
    if (typeof value !== 'string' || !/^#[0-9a-f]{6}$/i.test(value)) throw TypeError('Chart colors require #RRGGBB');
    return value.toLowerCase();
}
function mix(a,b,t) {
    return '#'+[1,3,5].map(i=>Math.round(parseInt(a.slice(i,i+2),16)*(1-t)+parseInt(b.slice(i,i+2),16)*t).toString(16).padStart(2,'0')).join('');
}
/** Stable distinct series shades rotate the active accent, with luminance
 * pulled towards the foreground so both light and dark themes remain readable. */
function seriesPalette(accent,foreground) {
    const rgb=[1,3,5].map(i=>parseInt(accent.slice(i,i+2),16)/255),hi=Math.max(...rgb),lo=Math.min(...rgb),d=hi-lo;
    let h=d===0?220:hi===rgb[0]?60*((rgb[1]-rgb[2])/d%6):hi===rgb[1]?60*((rgb[2]-rgb[0])/d+2):60*((rgb[0]-rgb[1])/d+4);
    const dark=parseInt(foreground.slice(1,3),16)>128;
    return Array.from({length:16},(_,i)=>{
        if(!i)return accent;
        const angle=((h+i*137.508)%360+360)%360,s=.58,l=dark?.64:.43,c=(1-Math.abs(2*l-1))*s,x=c*(1-Math.abs(angle/60%2-1)),m=l-c/2;
        const [r,g,b]=angle<60?[c,x,0]:angle<120?[x,c,0]:angle<180?[0,c,x]:angle<240?[0,x,c]:angle<300?[x,0,c]:[c,0,x];
        return '#'+[r,g,b].map(v=>Math.round((v+m)*255).toString(16).padStart(2,'0')).join('');
    });
}
export function resolveChartTheme(active = {}, overrides = {}) {
    if(!active||!overrides||Array.isArray(active)||Array.isArray(overrides))throw TypeError('Chart theme must be an object');
    const result={...DEFAULT_CHART_THEME};
    for(const key of keys)if(active[key]!=null)result[key]=chartColor(active[key]);
    for(const [key,value] of Object.entries(overrides)){
        if(key==='series')continue;
        if(!keys.has(key))throw TypeError('Unknown chart color '+key);
        result[key]=chartColor(value);
    }
    if(active.positive==null&&overrides.positive==null)result.positive=mix('#269c6b',result.foreground,.18);
    if(active.negative==null&&overrides.negative==null)result.negative=mix('#e05d70',result.foreground,.18);
    result.grid=mix(result.panel,result.border,.6);
    if(active.reducedMotion!=null&&typeof active.reducedMotion!=='boolean')throw TypeError('reducedMotion requires a boolean');
    result.reducedMotion=active.reducedMotion===true;
    result.series=seriesPalette(result.accent,result.foreground);
    const custom=overrides.series;
    if(custom!=null){
        if(!custom||typeof custom!=='object'||Object.keys(custom).length>32)throw TypeError('series colors require an array or name-to-color map (max 32)');
        result.seriesOverrides=Object.freeze(Object.fromEntries(Object.entries(custom).map(([name,value])=>[name,chartColor(value)])));
    }else result.seriesOverrides=Object.freeze({});
    return Object.freeze({...result,series:Object.freeze(result.series)});
}

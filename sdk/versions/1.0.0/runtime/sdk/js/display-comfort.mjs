/** NOAA fractional-year approximation. UTC milliseconds; explicit polar states.
 * Equations: https://gml.noaa.gov/grad/solcalc/solareqns.PDF */
const finite=(v,a,b)=>{if(!Number.isFinite(v)||v<a||v>b)throw RangeError('Display setting out of range');return v;};
const rad=Math.PI/180;
export function solarEvents({latitude,longitude,date=Date.now()}){
 finite(latitude,-90,90);finite(longitude,-180,180);const d=new Date(date);if(!Number.isFinite(d.getTime()))throw TypeError('Invalid solar date');
 // Pick the UTC calendar day containing local solar noon, including the date line.
 const local=new Date(d.getTime()+longitude*240000),year=local.getUTCFullYear(),day=Date.UTC(year,local.getUTCMonth(),local.getUTCDate()),days=(Date.UTC(year+1,0,1)-Date.UTC(year,0,1))/86400000;
 const gamma=2*Math.PI/days*((day-Date.UTC(year,0,1))/86400000),c=Math.cos,s=Math.sin;
 const equation=229.18*(0.000075+0.001868*c(gamma)-0.032077*s(gamma)-0.014615*c(2*gamma)-0.040849*s(2*gamma));
 const decl=0.006918-0.399912*c(gamma)+0.070257*s(gamma)-0.006758*c(2*gamma)+0.000907*s(2*gamma)-0.002697*c(3*gamma)+0.00148*s(3*gamma);
 const cosine=(c(90.833*rad)-s(latitude*rad)*s(decl))/(c(latitude*rad)*c(decl)),noon=day+(720-4*longitude-equation)*60000;
 if(cosine>1)return{state:'polar-night',sunrise:null,sunset:null,noon};if(cosine< -1)return{state:'polar-day',sunrise:null,sunset:null,noon};
 const offset=Math.acos(Math.max(-1,Math.min(1,cosine)))/rad*240000;return{state:'normal',sunrise:noon-offset,sunset:noon+offset,noon};
}
export function normalizeNightLight(value={}){
 if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['enabled','kelvin','brightness','schedule','pausedUntil'].includes(k)))throw TypeError('Invalid night light configuration');
 const {enabled=false,kelvin=4000,brightness=1,schedule=null,pausedUntil=0}=value;
 if(typeof enabled!=='boolean')throw TypeError('Night light enabled must be boolean');finite(kelvin,1000,6500);finite(brightness,0.25,1);finite(pausedUntil,0,8640000000000000);
 let normalized=null;if(schedule!==null){if(!schedule||typeof schedule!=='object'||Array.isArray(schedule)||Object.keys(schedule).some(k=>!['latitude','longitude'].includes(k)))throw TypeError('Solar schedule requires coordinates');normalized={latitude:finite(schedule.latitude,-90,90),longitude:finite(schedule.longitude,-180,180)};}
 return{enabled,kelvin,brightness,schedule:normalized,pausedUntil};
}
export function nightLightState(configuration,now=Date.now()){
 finite(now,0,8640000000000000);const config=normalizeNightLight(configuration),solar=config.schedule?solarEvents({...config.schedule,date:now}):null;
 const night=!solar||solar.state==='polar-night'||solar.state==='normal'&&(now<solar.sunrise||now>=solar.sunset),active=config.enabled&&now>=config.pausedUntil&&night;
 return{...config,active,effectiveKelvin:active?config.kelvin:6500,effectiveBrightness:active?config.brightness:1,solar};
}
/** RGB channel gains for a browser compositor approximation, not monitor calibration. */
export function nightLightGains(kelvin){
 finite(kelvin,1000,6500);
 const channels=k=>{const n=k/100;return[255,99.4708025861*Math.log(n)-161.1195681661,n<=19?0:138.5177312231*Math.log(n-10)-305.0447927307].map(v=>Math.max(0,Math.min(255,v)));};
 const rgb=channels(kelvin),white=channels(6500);return rgb.map((v,i)=>Math.max(0,Math.min(1,v/white[i])));
}
solarEvents.controlFactory=Object.freeze({topics:['desktopUtilities','dateTime'],constructor:'solarEvents({latitude,longitude,date:Date.now()})',methods:'Offline NOAA approximation returns {state:normal|polar-day|polar-night,sunrise,sunset,noon} in UTC milliseconds. Polar sunrise/sunset are null. Positive longitude is east; not a precision astronomical ephemeris.'});
normalizeNightLight.controlFactory=Object.freeze({topics:['desktopUtilities'],constructor:'normalizeNightLight({enabled:false,kelvin:4000,brightness:1,schedule:null,pausedUntil:0})',methods:'Complete bounded config; Kelvin 1000..6500, brightness 0.25..1, pause UTC milliseconds. schedule is null or {latitude,longitude}. Rejects unknown fields.'});
nightLightState.controlFactory=Object.freeze({topics:['desktopUtilities'],constructor:'nightLightState(configuration,now:Date.now())',methods:'Returns normalized config plus active, effectiveKelvin, effectiveBrightness and solar. Accounts for pause, local solar date and polar states.'});
nightLightGains.controlFactory=Object.freeze({topics:['desktopUtilities'],constructor:'nightLightGains(kelvin)',methods:'Returns bounded RGB multipliers for the browser compositor approximation. Use services.display to apply host-owned warmth; this helper does not calibrate a monitor.'});

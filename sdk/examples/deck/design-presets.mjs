import {DEFAULT_THEME_FORMAT} from '../../versions/1.0.0/runtime/sdk/js/presentation/theme-markup.mjs';
import {presentationThemeVariants,checkedPresentationTheme} from '../../versions/1.0.0/runtime/sdk/js/presentation/theme-contract.mjs';

// Original reference catalog, pinned to reference editor 0.8.0 commit
// 22db9261f9d1a214c296fd390173130c45d76d66, crates/model/src/theme.rs.
// Rice is the product's existing name for the reference's default theme.
const freeze=value=>{for(const item of Object.values(value))if(item&&typeof item==='object')freeze(item);return Object.freeze(value);};
const colors=value=>value.split(' ').map(c=>'#'+c);
const entries=[
 ['Rice','Inter','Inter','000000 FFFFFF 1E2B3C EDEBE7 2E6FD8 F26B1D 2EA36F 8A5CD8 D83F6B E3B21C 2E6FD8 7A5BA6'],
 ['Harbor','Merriweather','Open Sans','0B1F2A FFFFFF 12384D E4EEF2 1B7FA6 36B3A8 F2C14E F78154 5D576B 8AA29E 1B7FA6 5D576B'],
 ['Ember','Montserrat','Lato','1A1A1A FFFCF7 3D2B1F F6EDE3 D9480F F59F00 A61E4D 5C940D 1971C2 862E9C D9480F 862E9C'],
 ['Meadow','Merriweather','Open Sans','14281D FFFFFF 2D4A37 EEF5EE 4C9A2A A4C639 2F6F4E E9B44C 9B2915 50A2A7 2F6F4E 9B2915'],
 ['Nocturne','Inter','Inter','FFFFFF 121826 F2F4F8 1E2638 7C5CFF 00C2A8 FF6B9A FFC53D 4DA3FF B2F35F 7C5CFF FF6B9A'],
 ['Paper','Playfair Display','Lato','262626 FFFFFF 404040 F2F2F2 595959 8C8C8C C0504D 4F81BD 9BBB59 F79646 4F81BD 8064A2'],
 ['Coral Reef','Poppins','Nunito Sans','0F2A33 FFFFFF 0E4D64 FDF1EC FF6F59 254441 43AA8B B2B09B EF3054 3F88C5 3F88C5 EF3054'],
 ['Slate','Roboto','Roboto','111827 FFFFFF 374151 F3F4F6 6366F1 0EA5E9 10B981 F59E0B EF4444 8B5CF6 6366F1 8B5CF6'],
];
export const DESIGN_THEMES=freeze(entries.map(([name,major,minor,rgb])=>({id:name.toLowerCase().replaceAll(' ','-'),name,colorsName:name,colors:colors(rgb),fonts:{name,major:{latin:major},minor:{latin:minor}},format:structuredClone(DEFAULT_THEME_FORMAT)})));
// Gallery identity stays outside the document command's exact theme schema.
export function designThemeDefinition({id,...theme}){return checkedPresentationTheme(theme);}
export const DESIGN_COLORS=freeze([...DESIGN_THEMES.map(t=>({name:t.colorsName,colors:t.colors})),
 ...[
 ['Grayscale','000000 FFFFFF 000000 F8F8F8 DDDDDD B2B2B2 969696 808080 5F5F5F 4D4D4D 5F5F5F 919191'],
 ['Ocean','000000 FFFFFF 13293D E8F1F2 006494 247BA0 1B98E0 6CC1E8 13293D 0B3954 1B98E0 6CC1E8'],
 ['Sunset','2B2118 FFFFFF 6F1A07 FFF3E6 FF9F1C FFBF69 CB997E E76F51 F4A261 2A9D8F E76F51 2A9D8F'],
 ['Forest','1B1B1B FFFFFF 283618 FEFAE0 606C38 DDA15E BC6C25 283618 8A9A5B 4F772D 606C38 BC6C25'],
 ['Berry','1F0A1D FFFFFF 4A1942 F7E8F3 893168 C6426E EB6A8F 4A1942 7D7ABC 56A3A6 893168 56A3A6'],
 ].map(([name,rgb])=>({name,colors:colors(rgb)}))]);
export const DESIGN_FONTS=freeze([
 ['Inter','Inter','Inter'],['Classic','Merriweather','Open Sans'],['Geometric','Montserrat','Lato'],['Editorial','Playfair Display','Lato'],['Humanist','Merriweather','Open Sans'],['Friendly','Poppins','Nunito Sans'],['Neutral','Roboto','Roboto'],['Office Compatible','Carlito','Carlito'],['Times / Arial','Liberation Serif','Liberation Sans'],
].map(([name,major,minor])=>({name,major:{latin:major},minor:{latin:minor}})));
const shadow=(blur,distance,opacity)=>({shadow:{blur,distance,opacity,alignment:'b'}}),glow=radius=>({glow:{radius,opacity:.6}}),solid=()=>Array.from({length:3},()=>({kind:'solid'}));
export const DESIGN_EFFECTS=freeze([
 structuredClone(DEFAULT_THEME_FORMAT),
 {...structuredClone(DEFAULT_THEME_FORMAT),name:'Soft',lines:[.5,.75,1],effects:[shadow(16,1,.18),shadow(12,2,.22),shadow(10,3,.28)]},
 {...structuredClone(DEFAULT_THEME_FORMAT),name:'Crisp',fills:solid(),lines:[1,1.5,2.25],effects:[{},{},shadow(0,2,.55)]},
 {...structuredClone(DEFAULT_THEME_FORMAT),name:'Lift',effects:[{},shadow(8,6,.28),shadow(12,10,.35)]},
 {...structuredClone(DEFAULT_THEME_FORMAT),name:'Glow',effects:[{},glow(6),glow(12)]},
 {...structuredClone(DEFAULT_THEME_FORMAT),name:'Plain',fills:solid(),lines:[.75,.75,.75],effects:[{},{},{}]},
]);
export function designVariants(theme){
 const builtin=DESIGN_THEMES.find(t=>t.name===theme.name)|| (theme.name==='DeckCraft'?DESIGN_THEMES[0]:null);
 const anchor=builtin?{name:theme.name==='DeckCraft'?'DeckCraft':builtin.colorsName,colors:builtin.colors}:null;
 return presentationThemeVariants(theme,anchor);
}
/** UI and agent queries share this authored domain catalog. Gallery IDs and
 * formatName are descriptions, not fields of a theme mutation blueprint. */
export function deckDesignCatalog(){return structuredClone({
 themes:DESIGN_THEMES.map(({format,...theme})=>({...theme,formatName:format.name})),
 colors:DESIGN_COLORS,fonts:DESIGN_FONTS,effects:DESIGN_EFFECTS,defaultFormat:DEFAULT_THEME_FORMAT,
});}

import { BACKGROUND_PATTERNS, Button, CheckBox, ColorMenu, Column, Gallery, GradientEditor, Icons, Label, Menu,MenuItem,Message, NumberInput, RadioButton, Row, SceneBuilder, SceneSurface, ScrollView, State, checkedDrawingmlGradientValue, paintBackgroundFill, themeColorPaint, themeColorSwatches, PRESENTATION_STANDARD_COLORS, paintFromColorChoice, paintSelectionKey, colorWithOpacity, transformedColor,PICTURE_ALIGNMENTS,PICTURE_MIRRORS } from '@ricewm/app';
import { DEFAULT_BACKGROUND_SETTINGS,BACKGROUND_PICTURE_OPTIONS } from './background-contract.mjs';

/** Retained background task pane. The document remains the only source of truth;
 * each committed field patches the current resource under its ordinary history. */
export function createBackgroundProperties({ apply, choosePicture, close = () => {}, notice = () => {} }) {
    let scene = null, ready = false, pending = false, scope = null, target = null;
    const enabled = () => ready && !pending && Boolean(scene?.canSetBackground);
    const execute = async action => {
        if (!enabled()) return false;
        pending = true; refresh();
        try { return await action(); }
        catch (error) { notice(error.message); return false; }
        finally { pending = false; refresh(); }
    };
    const solid = new RadioButton({ name: 'background-solid', text: 'Solid fill' });
    const gradient = new RadioButton({ name: 'background-gradient', text: 'Gradient fill' });
    const pattern = new RadioButton({ name: 'background-pattern', text: 'Pattern fill' });
    const picture = new RadioButton({ name: 'background-picture', text: 'Picture or texture fill' });
    const inherited = new RadioButton({ name: 'background-follow-layout', text: 'Follow the layout' });
    const paletteOptions={columns:10,standardColors:PRESENTATION_STANDARD_COLORS};
    const themePaint=(slot,fallback)=>themeColorPaint(scene?.backgroundTheme,slot)??{color:fallback,opacity:1};
    const paintOf=({kind,...paint})=>paint;
    const colors = new ColorMenu({ name: 'background-color', label: 'Color', icon: 186, ...paletteOptions, allowNone: false, state:State.START_ALIGNED|State.TRAILING_ICON });
    const reset = new Button({ name: 'background-reset', text: 'Reset Background' });
    const all = new Button({ name: 'background-apply-all', text: 'Apply to All' });
    const insert = new Button({ name: 'background-insert-picture', text: 'Insert picture from file…', icon: 71 });
    const tile = new CheckBox({ name: 'background-tile', text: 'Tile picture as texture' });
    const fields = new Map();
    const patch = values => {
        if (!scene?.backgroundPicture) return false;
        const source = scene.backgroundPicture;
        const command = { type: 'set-slide-picture-background', slide: scene.slideId, asset: source.image.asset };
        for (const key of Object.keys(DEFAULT_BACKGROUND_SETTINGS)) command[key] = source[key];
        for(const key of BACKGROUND_PICTURE_OPTIONS)if(source[key]!==undefined)command[key]=source[key];
        return execute(() => apply([{ ...command, ...values }]));
    };
    const field = (key, label, unit, minimum, maximum, toUi, fromUi) => {
        const input = new NumberInput({ name: `background-${key}`, accessibleName: label, unit, minimum, maximum, precision: 2,
            onCommit: value => patch({ [key]: fromUi(value) }) });
        const row = new Row({ text: 'panel:form-control' }, [new Label({ text: label }), input]);
        fields.set(key, { input, row, toUi }); return row;
    };
    const pictureMenus=new Map();
    const choice=(key,label,choices)=>{
        const menu=new Menu({name:`background-picture-${key}`,text:Object.values(choices)[0],accessibleName:`Picture ${label.toLowerCase()}`,state:State.START_ALIGNED|State.TRAILING_ICON,icon:74});
        for(const [value,text]of Object.entries(choices))menu.append(new MenuItem({text})).on(Message.ACTIVATE,()=>void patch({[key]:value}));
        const row=new Row({text:'panel:form-control'},[new Label({text:label}),menu]);pictureMenus.set(key,{menu,row,choices});return row;
    };
    const pictureFields = new Column({ name: 'background-picture-properties', text: 'panel:tight-stack' }, [insert,
        field('opacity', 'Transparency', '%', 0, 100, value => (1 - value) * 100, value => 1 - value / 100), tile,
        field('offsetX', 'Offset X', 'pt', -750000, 750000, value => value * .75, value => value / .75),
        field('offsetY', 'Offset Y', 'pt', -750000, 750000, value => value * .75, value => value / .75),
        field('scaleX', 'Scale X', '%', 1, 10000, value => value * 100, value => value / 100),
        field('scaleY', 'Scale Y', '%', 1, 10000, value => value * 100, value => value / 100),
        choice('alignment','Alignment',PICTURE_ALIGNMENTS),choice('mirror','Mirror',PICTURE_MIRRORS),
    ]);
    const solidValue = () => scene?.backgroundFill?.kind === 'solid' ? scene.backgroundFill : {kind:'solid',color:scene?.background?.color??'#FFFFFF',opacity:scene?.background?.opacity??1};
    const setFill = fill => execute(() => apply([{type:'set-slide-fill-background',slide:scene.slideId,fill}]));
    const solidOpacity = new NumberInput({name:'background-solid-opacity',accessibleName:'Solid fill transparency',unit:'%',minimum:0,maximum:100,precision:2,
        onCommit:value=>setFill({kind:'solid',...colorWithOpacity(paintOf(solidValue()),1-value/100)})});
    const solidOpacityRow=new Row({text:'panel:form-control'},[new Label({text:'Transparency'}),solidOpacity]);
    const solidFields = new Column({name:'background-solid-properties',text:'panel:tight-stack'},[
        new Row({text:'panel:form-control'},[new Label({text:'Color'}),colors]),
        solidOpacityRow,
    ]);
    const defaultGradient = () => {const base=themePaint('bg2','#4472C4');return {kind:'linear',angle:90,scaled:false,stops:[{offset:0,...base},{offset:1,...transformedColor(base,[{kind:'lumMod',value:60000}])}]};};
    const gradientEditor = new GradientEditor({name:'background-gradient-editor',...paletteOptions,normalizeValue:checkedDrawingmlGradientValue,commitMode:'field',onCommit:value=>setFill({kind:'gradient',gradient:value})});
    const patternValue = () => scene?.backgroundFill?.kind === 'pattern' ? scene.backgroundFill : {kind:'pattern',preset:'pct20',foreground:themePaint('accent1','#4472C4'),background:themePaint('bg1','#FFFFFF')};
    const patternNames = ['5%','20%','50%','Light horizontal','Light vertical','Small grid','Dark downward diagonal','Wave','Dotted grid','Weave'];
    const patterns = new Gallery({name:'background-pattern-presets',presentation:'tiles',tileWidth:40,tileHeight:32,showSearch:false,showTitles:false,
        provider:async()=>({items:Object.keys(BACKGROUND_PATTERNS).map((id,i)=>({id,title:patternNames[i]})),next:null}),
        renderPreview:item=>{const preview=new SceneSurface({accessibleName:item.title});preview.submit(paintBackgroundFill(new SceneBuilder(36,28),{...patternValue(),preset:item.id},[0,0,36,28]).finish());return preview;},
        onSelect:item=>setFill({...patternValue(),preset:item.id})});
    patterns.status.visible=false;
    const foreground = new ColorMenu({name:'background-pattern-foreground',label:'Color',accessibleName:'Pattern foreground color',tooltip:'Pattern foreground color',icon:186,...paletteOptions,allowNone:false,state:State.START_ALIGNED|State.TRAILING_ICON});
    const patternBackground = new ColorMenu({name:'background-pattern-background',label:'Color',accessibleName:'Pattern background color',tooltip:'Pattern background color',icon:186,...paletteOptions,allowNone:false,state:State.START_ALIGNED|State.TRAILING_ICON});
    foreground.on(Message.VALUE_CHANGED,event=>void setFill({...patternValue(),foreground:paintFromColorChoice(patternValue().foreground,event.detail)}));
    patternBackground.on(Message.VALUE_CHANGED,event=>void setFill({...patternValue(),background:paintFromColorChoice(patternValue().background,event.detail)}));
    const patternFields = new Column({name:'background-pattern-properties',text:'panel:tight-stack'},[patterns,
        new Row({text:'panel:distribute:flush:gap=8'},[new Label({text:'Foreground',state:State.EXPANSIVE}),foreground]),
        new Row({text:'panel:distribute:flush:gap=8'},[new Label({text:'Background',state:State.EXPANSIVE}),patternBackground]),
    ]);
    let patternPreviewKey=null;
    const hideGraphics = new CheckBox({name:'background-hide-graphics',text:'Hide background graphics'});
    hideGraphics.on(Message.ACTIVATE,()=>{
        const command={type:'set-slide-background-graphics',slide:scene?.slideId,hidden:hideGraphics.getProperty('checked')};
        void execute(()=>apply([command]));
    });
    const sourceNotice = new Label({ name: 'background-source-description', text: '', visible: false });
    const readOnlyNotice = new Label({ name: 'background-read-only', text: 'This slide’s background can be viewed but cannot be edited.', visible: false });
    const done = new Button({ name: 'background-close', text: '', icon: Icons.Close, state: State.FLAT, tooltip: 'Close Format Background', accessibleName: 'Close Format Background' });
    done.on(Message.ACTIVATE, close);
    const header = new Row({ text: 'panel:fixed-height:28:flush:gap=4' }, [new Label({ name: 'background-heading', text: 'Format Background', state: State.EXPANSIVE }), done]);
    const body = new Column({name:'background-fill-properties',text:'panel:tight-stack'},[
        readOnlyNotice, new Label({ text: 'Fill' }), solid, gradient, picture, pattern, inherited, solidFields, gradientEditor, pictureFields, patternFields, sourceNotice, hideGraphics,
        new Row({ text: 'panel:actions' }, [all, reset]),
    ]);
    const control = new Column({ name: 'background-properties', text: 'panel:flush:gap=0',state:State.EXPANSIVE }, [header,
        new ScrollView({name:'background-fill-scroll',state:State.EXPANSIVE},[body])]);
    solid.on(Message.ACTIVATE, () => {
        // Do not flatten the current paint when opening the pane or refreshing it.
        if (!scene?.background || scene?.backgroundKind && scene.backgroundKind !== 'solid' || scene?.backgroundPicture || scene?.backgroundGradient)
            void (scene.canSetBackgroundFill ? setFill({kind:'solid',...themePaint('bg1','#FFFFFF')}) : execute(() => apply([{ type: 'set-slide-background', slide: scene.slideId, color: '#FFFFFF' }])));
    });
    gradient.on(Message.ACTIVATE,()=>{if(scene?.backgroundFill?.kind!=='gradient'&&scene?.canSetBackgroundFill)void setFill({kind:'gradient',gradient:defaultGradient()});});
    pattern.on(Message.ACTIVATE,()=>{if(scene?.backgroundFill?.kind!=='pattern'&&scene?.canSetBackgroundFill)void setFill(patternValue());});
    const pick = () => {
        if (typeof choosePicture !== 'function') return false;
        const slide = scene?.slideId;
        return execute(() => choosePicture({ slide, ...DEFAULT_BACKGROUND_SETTINGS }));
    };
    picture.on(Message.ACTIVATE, () => { if(scene?.backgroundPicture&&scene?.backgroundKind==='layout')void patch({});else if (!scene?.backgroundPicture) void pick(); });
    insert.on(Message.ACTIVATE, pick);
    colors.on(Message.VALUE_CHANGED, event => void (scene?.canSetBackgroundFill ? setFill({kind:'solid',...paintFromColorChoice(paintOf(solidValue()),event.detail)}) : execute(() => apply([{ type: 'set-slide-background', slide: scene.slideId, color: event.detail.value }]))));
    const restore = () => execute(() => apply([{ type: 'reset-slide-background', slide: scene.slideId }]));
    inherited.on(Message.ACTIVATE, restore); reset.on(Message.ACTIVATE, restore);
    tile.on(Message.ACTIVATE, () => void patch({ mode: tile.getProperty('checked') ? 'tile' : 'stretch' }));
    all.on(Message.ACTIVATE, () => {
        if (!all.enabled) return;
        if (scene.backgroundPicture && scene.backgroundKind!=='layout') void patch({ slide: null });
        else if(scene.canSetBackgroundFill)void execute(()=>apply([{type:'copy-slide-background',source:scene.slideId,slide:null}]));
        else void execute(() => apply([{ type: 'set-slide-background', slide: null, color: scene.background.color }]));
    });
    function refresh() {
        readOnlyNotice.visible = ready && Boolean(scene) && !scene.canSetBackground;
        const active=enabled(),enhanced=Boolean(scene?.canSetBackgroundFill),same=target===JSON.stringify([scope,scene?.slideId]);
        const kind=scene?.backgroundKind,hasPicture=Boolean(scene?.backgroundPicture)&&(kind===undefined||kind==='picture');
        const hasGradient=kind==='gradient'||kind===undefined&&Boolean(scene?.backgroundGradient),hasPattern=kind==='pattern';
        const hasSolid=!hasPicture&&!hasGradient&&!hasPattern&&Boolean(scene?.background)&&(kind===undefined||kind==='solid');
        for(const button of [solid,inherited,reset])button.enabled=active;
        gradient.enabled=pattern.enabled=active&&enhanced;
        picture.enabled=insert.enabled=active&&typeof choosePicture==='function';
        solid.setProperty('checked',hasSolid);gradient.setProperty('checked',hasGradient);pattern.setProperty('checked',hasPattern);picture.setProperty('checked',hasPicture);
        inherited.setProperty('checked',kind==='layout'||kind===undefined&&!hasSolid&&!hasPicture&&!hasGradient);
        solidFields.visible=hasSolid;pictureFields.visible=hasPicture;patternFields.visible=hasPattern;
        gradientEditor.visible=hasGradient&&scene?.backgroundFill?.kind==='gradient';
        const swatches=themeColorSwatches(scene?.backgroundTheme);
        for(const menu of [colors,foreground,patternBackground])menu.palette.setThemeColors(swatches);
        gradientEditor.setThemeColors(swatches);
        gradientEditor.enabled=active&&enhanced;
        if(gradientEditor.visible)gradientEditor.setValue(scene.backgroundFill.gradient,{preserveDraft:same});
        else if(!same)gradientEditor.setValue(defaultGradient());
        colors.enabled=active&&hasSolid;colors.setValue(solidValue().color,{preserveDraft:same,selectionKey:paintSelectionKey(solidValue(),swatches)});
        solidOpacityRow.visible=enhanced;solidOpacity.enabled=active&&hasSolid&&enhanced;
        solidOpacity.setValue((1-solidValue().opacity)*100,{preserveDraft:same});
        tile.enabled=active&&hasPicture;tile.setProperty('checked',scene?.backgroundPicture?.mode==='tile');
        for(const[key,{input,row,toUi}]of fields){
            row.visible=key==='opacity'||scene?.backgroundPicture?.mode==='tile';input.enabled=active&&hasPicture;
            input.setValue(hasPicture?toUi(scene.backgroundPicture[key]):null,{preserveDraft:same});
        }
        for(const [key,{menu,row,choices}]of pictureMenus){
            row.visible=hasPicture&&scene?.backgroundPicture?.mode==='tile'&&Boolean(scene?.canSetPictureFillOptions);
            menu.enabled=active&&hasPicture&&Boolean(scene?.canSetPictureFillOptions);
            const selected=scene?.backgroundPicture?.[key]??(key==='alignment'?'tl':'none');menu.text=choices[selected]??'';
            for(const item of menu.children)item.setProperty('checked',item.text===choices[selected]);
        }
        patterns.enabled=foreground.enabled=patternBackground.enabled=active&&enhanced&&hasPattern;
        if(hasPattern){const value=patternValue();patterns.setSelection(value.preset);foreground.setValue(value.foreground.color,{preserveDraft:same,selectionKey:paintSelectionKey(value.foreground,swatches)});patternBackground.setValue(value.background.color,{preserveDraft:same,selectionKey:paintSelectionKey(value.background,swatches)});
            const key=JSON.stringify([value.foreground,value.background]);if(patternPreviewKey!==key){patternPreviewKey=key;void patterns.search('').catch(()=>{});}}
        hideGraphics.visible=Boolean(scene?.canSetBackgroundGraphics);hideGraphics.enabled=active&&Boolean(scene?.canSetBackgroundGraphics);
        hideGraphics.setProperty('checked',Boolean(scene?.hideBackgroundGraphics));
        all.enabled=active&&(hasPicture||enhanced&&kind!=='layout'||hasSolid&&scene.background.opacity===1);
        sourceNotice.visible=(hasGradient&&!gradientEditor.visible)||(hasPattern&&!scene?.backgroundFill)||hasSolid&&!enhanced&&scene.background.opacity!==1;
        sourceNotice.text=hasGradient||hasPattern?'This source fill is preserved. Choose a fill type to replace it.':'This fill has transparency. Choosing a color replaces the fill.';
        target=JSON.stringify([scope,scene?.slideId]);
    }
    return Object.freeze({ control, body, setState({ current = scene, usable = ready, documentScope = scope } = {}) {
        scene = current; ready = usable; scope = documentScope; refresh();
    } });
}

import { ContextMenu, MenuItem, Message } from '@ricewm/app';

/** Native owner-scoped commands keep the clicked slide identity across activation. */
export function slideContext(owner, { target, invoke, available, formatBackground=null, clipboard=null, batchAvailable=()=>false, sectionAvailable=()=>false, selection = () => target ? [target.slide] : [], hidden = () => false }) {
    const menu = new ContextMenu({ name: target ? `slide-context-${target.index}` : 'slides-empty-context' });
    const commands = [...[['cut','Cut',22],['copy','Copy',23],['paste','Paste',24]].filter(([kind])=>typeof clipboard?.[kind]==='function'&&(target||kind==='paste')).map(([kind,text,icon])=>['clipboard-'+kind,text,icon]),['insert-slide', 'New slide', 10], ...(target
        ? [['duplicate-slide', 'Duplicate slide', 23], ['delete-slide', 'Delete slide', 25], ['set-slides-hidden', 'Hide Slide', 104]] : []),['add-section','Add Section',null]];
    for (const [type, text, icon] of commands) {
        const item = new MenuItem({ name: `${menu.name}-${type}`, text, icon, enabled: available() });
        item.on(Message.ACTIVATE, () => { if (!available()||!item.enabled) return;
            if(type.startsWith('clipboard-')){const kind=type.slice(10);if(clipboard?.can(kind))void clipboard[kind]();return;}
            if(type==='add-section'&&!sectionAvailable())return;
            const slides = [...selection()];
            const scoped = target && (type === 'delete-slide' || type === 'set-slides-hidden' || type==='duplicate-slide'&&slides.length>1)
                ? { ...target, slides, ...(type === 'set-slides-hidden' ? { hidden: !slides.every(hidden) } : {}) } : target;
            void invoke(type, scoped); });
        menu.append(item);
    }
    if (target && typeof formatBackground === 'function') {
        const item = new MenuItem({ name: `${menu.name}-format-background`, text: 'Format Background…', icon: 186 });
        item.on(Message.ACTIVATE, () => {
            if (available() && item.enabled) void formatBackground({ ...target });
        });
        menu.append(item);
    }
    menu.attachTo(owner);
    return { update() {
        const name = target ? `slide-context-${target.index}` : 'slides-empty-context';
        if (name !== menu.name) {
            for (const item of menu.children) item.setProperty('name', name + item.name.slice(menu.name.length));
            menu.setProperty('name', name);
        }
        const slides = selection(), allHidden = slides.length > 0 && slides.every(hidden);
        for (const item of menu.children) {
            item.setProperty('enabled', available());
            if(item.name.includes('-clipboard-'))item.enabled=available()&&clipboard.can(item.name.split('-clipboard-').at(-1));
            if(item.name.endsWith('-add-section')){item.visible=sectionAvailable();item.enabled=available()&&sectionAvailable();}
            if(item.name.endsWith('-duplicate-slide')){item.text=slides.length>1?'Duplicate selected slides':'Duplicate slide';item.enabled=available()&&slides.length<=32&&(slides.length<=1||batchAvailable());item.setProperty('tooltip',slides.length>32?'Duplicate up to 32 slides per transaction':item.text);}
            if (item.name.endsWith('-delete-slide')) item.text = slides.length > 1 ? 'Delete selected slides' : 'Delete slide';
            if (item.name.endsWith('-set-slides-hidden')) {
                item.text = `${allHidden ? 'Unhide' : 'Hide'} ${slides.length > 1 ? 'Selected Slides' : 'Slide'}`;
                item.setProperty('tooltip', item.text);
            }
        }
    } };
}

export function sectionContext(owner,{section,sections,available,invoke,rename}) {
    const menu=new ContextMenu({name:'section-context-'+owner.name}),items=new Map();
    for(const [type,text]of [['rename-section','Rename Section…'],['add-section','Add Section'],['remove-section','Remove Section'],['remove-section-slides','Remove Section & Slides'],['move-section-earlier','Move Section Up'],['move-section-later','Move Section Down']]) {
        const item=new MenuItem({name:menu.name+'-'+type,text});items.set(type,item);menu.append(item);
        item.on(Message.ACTIVATE,()=>{const current=section();if(!current||!available()||!item.enabled)return;if(type==='rename-section')void Promise.resolve(rename()).catch(error=>owner.setProperty('tooltip',String(error?.message||error)));else void invoke(type,{section:current.id});});
    }
    menu.attachTo(owner);
    const update=()=>{const current=section(),all=sections(),index=all.findIndex(s=>s.id===current?.id);for(const [type,item]of items)item.enabled=available()&&index>=0&&(type!=='move-section-earlier'||index>0)&&(type!=='move-section-later'||index+1<all.length);};update();
    return {update};
}

/** The native browser boundary supplies one semantic double-click (button 6).
 * A background is a pointer surface, never an activatable command button. */
export function emptySlideDoubleClick(target, invoke, available) {
    let down = false, completed = null;
    const reset = () => { down = false; completed = null; };
    target.on(Message.POINTER, ({ detail }) => {
        if(detail.kind==='down'&&detail.button===6){
            const recent=completed!==null&&performance.now()-completed<=450;
            reset();if(recent&&available())void invoke('insert-slide');return;
        }
        if(detail.kind==='down'&&detail.button===1&&available())down=true;
        else if(detail.kind==='up'&&detail.button===1){completed=down?performance.now():null;down=false;}
        else if(detail.kind!=='move')reset();
    });
    target.on(Message.KEY, reset);
    target.on(Message.BLUR, reset);
    return reset;
}

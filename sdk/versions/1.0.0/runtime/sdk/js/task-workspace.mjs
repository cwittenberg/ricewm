import {Workspace} from './workspace.mjs';
import {Button,CheckBox,Column,Container,Row,ScrollView,TextField,TextEditor,Label,Tooltip} from './controls.mjs';
import {DatePicker} from './date-time-picker.mjs';
import {Message} from './messages.mjs';
import {State} from './control.mjs';
import {Icons} from './icons.mjs';
import {taskStore,visibleTasks,addTask,civilDate,today} from './task-workspace-data.mjs';
const flat=State.ENABLED|State.FLAT;
const keyOf=e=>e.detail?.pressed===false?null:e.detail?.key||({36:'Enter',9:'Escape',111:'ArrowUp',116:'ArrowDown'}[e.detail?.keyCode]);
const field=(name)=>new TextField({name,accessibleName:name});
/** Complete native task workspace with app-owned storage and real controls.
 * A requested app can compose this proven workflow and customize its palette.
 * It does not acquire host authority or expose account/cloud integrations. */
export async function createTaskWorkspace({services},{width=1080,height=720,colors={},storageKey='tasks',initialView='my-day'}={}){
    if(!services?.settings||typeof storageKey!=='string'||!storageKey||storageKey.length>120||!['my-day','important','planned','tasks'].includes(initialView))throw TypeError('Task workspace needs settings.get/set, a storage key and a supported initial view');
    let store,loadError='';try{store=taskStore(await services.settings.get({key:storageKey}));}catch(error){store=taskStore(null);loadError=error.message;}
    let list=initialView,selected=null,query='',sort='created',status=loadError,nav=true,narrowNav=false,pendingDelete=false,loaded=null;
    const search=field('search'),quick=field('quick-add'),listName=field('list-name'),title=field('task-title'),due=new DatePicker({name:'task-due',label:'Due date',value:''}),stepInput=field('step-title');
    const note=new TextEditor({name:'task-note',accessibleName:'Task note',rows:5});
    const tasks=new Column({text:'panel:fixed:flush:gap=2',state:flat}),steps=new Column({text:'panel:fixed:flush:wrap-controls:gap=2',state:flat});
    let viewport;
    let view;
    const drafts=new Map(),rowCache=new Map(),stepCache=new Map(),navCache=new Map();
    const navigation=new Column({text:'panel:fixed:flush:intrinsic-flow:gap=6',state:flat}),navigationScroll=new ScrollView({name:'navigation-scroll',text:'panel:flush',state:flat|State.EXPANSIVE},[navigation]);
    const current=()=>store.tasks.find(t=>t.id===selected);
    const action=(name,text,handler)=>{const b=new Button({name,text,accessibleName:text,state:flat|State.START_ALIGNED},[new Tooltip({text})]);b.on(Message.ACTIVATE,handler);return b;};
    const refresh=()=>{renderRows();renderNavigation();view?.refresh();};
    // Serialize writes and commit the new state only after durable storage succeeds.
    let saving=Promise.resolve();
    const change=mutation=>saving=saving.then(async()=>{if(loadError)throw Error(loadError);const next=structuredClone(store);mutation(next);await services.settings.set({key:storageKey,value:next});store=next;status='';refresh();}).catch(error=>{status=error.message;refresh();});
    const cacheDetails=()=>{if(loaded)drafts.set(loaded,{title:title.value,due:due.value,note:note.value});};
    const choose=id=>{cacheDetails();selected=id;pendingDelete=false;loadDetails();refresh();};
    function loadDetails(){const t=current();loaded=t?.id||null;const draft=drafts.get(t?.id)||t;title.value=draft?.title||'';due.value=draft?.due||'';note.value=draft?.note||'';stepInput.value='';}
    function renderRows(){
        const visible=visibleTasks(store,list,query,sort),ids=new Set(visible.map(t=>t.id));
        for(const row of [...tasks.children])if(!ids.has(row.taskId))tasks.remove(row,{destroy:false});
        for(const t of visible){
            let row=rowCache.get(t.id);
            if(!row){
                const taskId=t.id,completion=new CheckBox({name:'complete-'+t.id,accessibleName:'Complete task',state:flat});
                completion.on(Message.ACTIVATE,()=>change(s=>{const task=s.tasks.find(x=>x.id===taskId);task.done=!task.done;}));
                const caption=action('select-'+t.id,t.title,()=>choose(taskId));caption.setProperty('state',flat|State.START_ALIGNED|State.EMPHASIZED);
                const star=action('important-'+t.id,'',()=>change(s=>{const task=s.tasks.find(x=>x.id===taskId);task.important=!task.important;}));
                const listLabel=new Label({name:'list-'+t.id,state:State.FLAT}),dueLabel=new Label({name:'due-'+t.id,state:State.FLAT}),stepLabel=new Label({name:'steps-'+t.id,state:State.FLAT}),completedLabel=new Label({name:'completed-'+t.id,text:'Completed',state:State.FLAT});
                const column=new Column({text:'panel:flush:wrap-controls:gap=0',state:flat|State.EXPANSIVE},[caption,listLabel,dueLabel,stepLabel,completedLabel]);
                const leading=control=>new Column({text:'panel:fixed:flush:width=40',state:flat},[new Container({text:'panel:fixed:flush:accessory-height=40',state:flat},[control])]);
                row=new Container({name:'task-card-'+t.id,text:'panel:content-card:fixed:padding=8',state:State.ENABLED},[new Row({text:'panel:fixed:flush:wrap-controls:gap=4',state:flat},[leading(completion),column,leading(star)])]);
                row.taskId=t.id;row.refresh=()=>{
                    const task=store.tasks.find(x=>x.id===taskId);if(!task)return;
                    caption.text=task.title;caption.setProperty('selected',selected===taskId);
                    completion.setProperty('checked',task.done);completion.setProperty('accessibleName',task.done?'Mark incomplete':'Complete task');
                    star.setProperty('icon',task.important?Icons.Star:Icons.StarOutline);star.setProperty('accessibleName',task.important?'Remove importance':'Mark important');star.children[0].text=star.getProperty('accessibleName');
                    listLabel.text=store.lists.find(l=>l.id===task.list)?.title||'';
                    dueLabel.text=task.due?'Due '+task.due:'';dueLabel.visible=!!task.due;
                    stepLabel.text=`${task.steps.filter(s=>s.done).length} of ${task.steps.length} steps`;stepLabel.visible=!!task.steps.length;
                    completedLabel.visible=task.done;
                };rowCache.set(t.id,row);
            }
            if(!row.parent)tasks.append(row);row.refresh();
        }
        for(const [id,row]of rowCache)if(!store.tasks.some(t=>t.id===id)){if(row.parent)row.parent.remove(row,{destroy:true});else row.destroy();rowCache.delete(id);}
        // Reordering changes native indexes, without changing row/control identities.
        if(tasks.children.some((r,i)=>r.taskId!==visible[i]?.id)){for(const r of [...tasks.children])tasks.remove(r,{destroy:false});for(const t of visible)tasks.append(rowCache.get(t.id));}
        const t=current(),shown=new Set();
        t?.steps.forEach((s,i)=>{
            const key=t.id+':'+i;shown.add(key);let c=stepCache.get(key);
            if(!c){const taskId=t.id;c=new CheckBox({name:'step-'+i,state:flat});c.on(Message.ACTIVATE,()=>change(data=>{const step=data.tasks.find(x=>x.id===taskId).steps[i];step.done=!step.done;}));stepCache.set(key,c);}
            c.text=s.title;c.setProperty('accessibleName',s.title);c.setProperty('checked',s.done);if(!c.parent)steps.append(c);
        });
        for(const c of [...steps.children])if(!shown.has([...stepCache].find(([,control])=>control===c)?.[0]))steps.remove(c,{destroy:false});
    }
    function renderNavigation(){
        const destinations=[['my-day','My Day',Icons.Sun],['important','Important',Icons.Star],['planned','Planned',Icons.Calendar],['tasks','Tasks',Icons.ViewList],...store.lists.map(l=>[l.id,l.title,l.id==='personal'?Icons.Home:l.id==='work'?Icons.Briefcase:Icons.ViewList])];
        for(const [index,[id,caption,glyph]]of destinations.entries()){
            let row=navCache.get(id);
            if(!row){
                if(index===4)navigation.append(new Column({text:'panel:fixed:flush:tight-stack:multiline',state:flat},[new Label({name:'lists-heading',text:'My lists',state:State.FLAT|State.EMPHASIZED})]));
                const button=action('nav-'+id,caption,()=>openList(id));button.setProperty('icon',null);button.setProperty('state',flat|State.START_ALIGNED|State.EXPANSIVE);
                const count=new Label({name:'count-'+id,state:State.FLAT|State.END_ALIGNED});
                const countSlot=new Column({text:'panel:fixed:flush:width=32',state:flat},[new Column({text:'panel:fixed:flush:tight-stack:multiline:center-content:accessory-height=32',state:flat},[count])]);
                const icon=new Label({icon:glyph,state:flat});icon.on(Message.POINTER,event=>{if(event.detail?.kind==='up'&&event.detail?.buttons===1)openList(id);});
                const iconSlot=new Column({text:'panel:fixed:flush:width=40',state:flat},[new Container({text:'panel:fixed:flush:accessory-height=32',state:flat},[icon])]);
                row=new Row({text:'panel:fixed:flush:wrap-controls:gap=4',state:flat},[iconSlot,button,countSlot]);row.count=count;navCache.set(id,row);navigation.append(row);
                button.on(Message.KEY,event=>{const key=keyOf(event);if(!['ArrowUp','ArrowDown'].includes(key))return;event.preventDefault();const ids=['my-day','important','planned','tasks',...store.lists.map(l=>l.id)],index=ids.indexOf(id),next=ids[(index+(key==='ArrowDown'?1:ids.length-1))%ids.length];openList(next);const destination=navCache.get(next).children[1];navigationScroll.scrollTo(destination);destination.setProperty('focused',true);});
            }
            const button=row.children[1],count=row.count;button.text=caption;button.setProperty('selected',list===id&&!query);const n=visibleTasks(store,id).filter(t=>!t.done).length;count.text=n?String(n):'';
        }
    }
    search.on(Message.VALUE_CHANGED,()=>{cacheDetails();query=search.value;selected=null;refresh();});
    const add=()=>{const value=quick.value;return change(s=>{const t=addTask(s,value,list);selected=t.id;}).then(()=>{if(!status){quick.value='';loadDetails();refresh();}});};
    quick.on(Message.KEY,event=>{if(keyOf(event)==='Enter'){event.preventDefault();return add();}});
    for(const editor of [title,note])editor.on(Message.KEY,event=>{
        if(keyOf(event)!=='Escape')return;
        event.preventDefault();const previous=selected;cacheDetails();selected=null;pendingDelete=false;refresh();
        const opener=rowCache.get(previous)?.find('select-'+previous)||quick;opener.setProperty('focused',true);
    });
    const openList=id=>{cacheDetails();list=id;query='';search.value='';selected=null;narrowNav=false;pendingDelete=false;refresh();};
    const saveDetails=()=>{const t=current();if(!t)return;const text=title.value.trim(),date=due.value,noteText=note.value;if(!text){status='Enter a task title.';refresh();return;}if(due.field.getProperty('invalid')||!civilDate(date)){status='Use a valid date: YYYY-MM-DD.';refresh();return;}return change(s=>{Object.assign(s.tasks.find(x=>x.id===t.id),{title:text,due:date,note:noteText});drafts.delete(t.id);});};
    const done=new CheckBox({name:'detail-done',accessibleName:'Complete task',state:flat});done.on(Message.ACTIVATE,()=>change(s=>{const t=s.tasks.find(x=>x.id===selected);t.done=!t.done;}));
    const important=action('detail-important','Mark important',()=>change(s=>{const t=s.tasks.find(x=>x.id===selected);t.important=!t.important;}));important.setProperty('icon',Icons.Star);
    const day=action('detail-day','Add to My Day',()=>change(s=>{const t=s.tasks.find(x=>x.id===selected);t.day=!t.day;}));day.setProperty('icon',Icons.Sun);
    const addStep=action('add-step','Add step',()=>change(s=>{const text=stepInput.value.trim();if(!text)throw Error('Enter a step.');const t=s.tasks.find(x=>x.id===selected);if(text.length>300||t.steps.length>=100)throw Error('Step limit exceeded.');t.steps.push({title:text,done:false});stepInput.value='';}));
    const heading=new Label({name:'list-heading',state:State.FLAT|State.EMPHASIZED|State.DISPLAY}),subtitle=new Label({name:'list-subtitle',state:State.FLAT}),empty=new Label({name:'empty-title',state:State.FLAT|State.EMPHASIZED});
    const sortButton=action('sort','Sort: added',()=>{sort=sort==='created'?'title':sort==='title'?'due':'created';refresh();});
    const addButton=action('add-task','Add task',add);addButton.setProperty('state',State.ENABLED|State.ACCENT);
    const compact=(children,properties={})=>new Column({text:'panel:fixed:flush:tight-stack:multiline:gap=6',state:flat,...properties},children);
    const mainHeading=new Row({text:'panel:fixed:flush:wrap-controls:gap=8',state:flat},[compact([heading],{text:'panel:flush:tight-stack:multiline:gap=6',state:flat|State.EXPANSIVE}),new Column({text:'panel:fixed:flush:width=130',state:flat},[new Container({text:'panel:fixed:flush:accessory-height=36',state:flat},[sortButton])])]);
    quick.setProperty('state',State.ENABLED|State.EXPANSIVE);
    const quickRow=new Row({text:'panel:fixed:flush:accessory-height=40:gap=8',state:flat},[quick,new Container({text:'panel:fixed:flush:width=96:accessory-height=40',state:flat},[addButton])]);
    const mainBody=new Column({text:'panel:fixed:flush:intrinsic-flow:gap=20',state:flat},[compact([mainHeading,subtitle]),compact([new Label({name:'quick-label',text:'Add a task',state:State.FLAT}),quickRow]),tasks,compact([empty])]);
    viewport=new ScrollView({name:'task-list',text:'panel:flush',state:flat|State.EXPANSIVE},[mainBody]);
    const label=text=>new Label({text,state:State.FLAT|State.EMPHASIZED});
    const fieldRow=children=>new Row({text:'panel:fixed:flush:accessory-height=40:gap=8',state:flat},children);
    const accessory=control=>new Container({text:'panel:fixed:flush:width=96:accessory-height=40',state:flat},[control]);
    title.setProperty('state',State.ENABLED|State.EXPANSIVE);stepInput.setProperty('state',State.ENABLED|State.EXPANSIVE);due.field.setProperty('state',State.ENABLED|State.EXPANSIVE);
    due.inputRow.setProperty('text','panel:fixed:flush:accessory-height=40:gap=8');due.disclosure.text='Choose';due.disclosure.parent.setProperty('text','panel:fixed:flush:width=96:accessory-height=40');
    const searchRow=new Row({text:'panel:fixed:flush:accessory-height=36:gap=12',state:flat},[new Column({text:'panel:fixed:flush:width=72:tight-stack:multiline:center-content',state:flat},[new Label({name:'search-label',text:'Search',state:State.FLAT})]),search]);
    search.setProperty('state',State.ENABLED|State.EXPANSIVE);
    const titleHeader=new Column({text:'panel:fixed:flush:tight-stack:gap=6',state:flat},[label('Task title'),fieldRow([new Container({text:'panel:fixed:flush:width=32:accessory-height=40',state:flat},[done]),title])]);
    const options=new Column({text:'panel:fixed:flush:gap=4',state:flat},[important,day]);
    const stepGroup=new Column({text:'panel:fixed:flush:tight-stack:wrap-controls:gap=6',state:flat},[label('Steps'),fieldRow([stepInput,accessory(addStep)]),steps]);
    const noteGroup=new Column({text:'panel:fixed:flush:tight-stack:gap=6',state:flat},[label('Note'),new Container({text:'panel:fixed-height:128:flush',state:flat},[note])]);
    const detailBody=new Column({text:'panel:fixed:flush:tight-stack:gap=12',state:flat},[options,due,stepGroup,noteGroup]);
    const detailScroll=new ScrollView({name:'detail-scroll',text:'panel:flush',state:flat|State.EXPANSIVE},[detailBody]);
    // Validation belongs to the fixed footer, keeping input groups stationary.
    due.remove(due.feedback,{destroy:false});due.feedback.on(Message.CHANGE,()=>view?.refresh());
    const cancelDetails=()=>{drafts.delete(selected);loadDetails();pendingDelete=false;status='';refresh();};
    const save=action('save-details','Save changes',saveDetails);save.setProperty('state',State.ENABLED|State.ACCENT);
    const cancel=action('cancel-details','Cancel',cancelDetails);cancel.setProperty('state',State.ENABLED);
    const saveSlot=new Container({text:'panel:fixed:flush:width=126:accessory-height=40',state:flat},[save]),cancelSlot=new Container({text:'panel:fixed:flush:width=126:accessory-height=40',state:flat},[cancel]);
    const footerActions=new Row({text:'panel:fixed:flush:accessory-height=40:gap=8',state:flat},[saveSlot,cancelSlot]);
    const remove=action('delete-task','Delete task',()=>{if(!pendingDelete){pendingDelete=true;view.refresh();return;}return change(s=>{s.tasks=s.tasks.filter(x=>x.id!==selected);selected=null;pendingDelete=false;});});remove.setProperty('icon',Icons.EditDelete);remove.setProperty('destructive',true);
    renderRows();renderNavigation();
    view=new Workspace({width,height,colors,render(v){
        const w=v.width,h=v.height,rail=w>=700?(nav?256:0):narrowNav?Math.min(256,w):0,detail=current(),inspector=detail?(w>=1000?300:w>=700?280:w):0,detailOnly=!!detail&&w<1000;
        const x=rail+24,right=w-(detailOnly?0:inspector)-24,bodyW=Math.max(160,right-x);
        v.panel([0,0,w,52]);v.rule([0,52],[w,52]);
        v.action('navigation','',[18,8,40,36],()=>{if(w>=700)nav=!nav;else narrowNav=!narrowNav;v.refresh();},{title:'Toggle navigation',icon:Icons.Menu});
        v.place('search-row',searchRow,[Math.max(x,76),8,w-Math.max(x,76)-24,36]);
        if(rail&&!detailOnly){
            v.panel([0,52,rail,h-52]);v.rule([rail,52],[rail,h]);
            v.place('navigation-list',navigationScroll,[18,76,rail-36,Math.max(120,h-220)]);
            v.label('new-list-label','New list',[18,h-114,rail-36,26]);v.place('list-name',listName,[18,h-82,rail-68,34]);v.action('new-list','',[rail-46,h-82,32,34],()=>change(s=>{const text=listName.value.trim();if(!text)throw Error('Enter a list name.');if(text.length>120||s.lists.length>=100)throw Error('List name or list limit exceeded.');const l={id:'list-'+s.next++,title:text};s.lists.push(l);list=l.id;listName.value='';selected=null;}),{title:'Create list',icon:Icons.ObjectAdd});
        }
        if(!detailOnly){
            heading.text=query?'Search results':({'my-day':'My Day',important:'Important',planned:'Planned',tasks:'Tasks'}[list]||store.lists.find(l=>l.id===list)?.title||'Tasks');
            heading.setProperty('state',State.FLAT|State.EMPHASIZED|(heading.text.length<=48?State.DISPLAY:0));
            sortButton.text='Sort: '+({created:'added',title:'title',due:'due date'}[sort]);
            const count=visibleTasks(store,list,query).filter(t=>!t.done).length;
            subtitle.text=`${count} active ${count===1?'task':'tasks'}`;
            const rows=visibleTasks(store,list,query);tasks.visible=!!rows.length;empty.visible=!rows.length;empty.text=query?'No matching tasks':'No tasks yet';
            v.place('task-list',viewport,[x,76,bodyW,h-100]);
        }
        if(detail){
            const dx=detailOnly?0:w-inspector,dw=detailOnly?w:inspector;v.panel([dx,52,dw,h-52]);v.rule([dx,52],[dx,h]);
            const left=dx+20,iw=dw-40;
            v.action('close-details',detailOnly?'‹ Back':'',[detailOnly?dx+dw-120:dx+dw-52,64,detailOnly?100:32,34],()=>{cacheDetails();selected=null;pendingDelete=false;refresh();},{title:'Close task details',icon:detailOnly?null:Icons.Close});
            v.place('task-header',titleHeader,[left,64,iw,72]);
            done.setProperty('checked',detail.done);done.setProperty('accessibleName',detail.done?'Mark incomplete':'Complete task');important.text=detail.important?'Important':'Mark important';day.text=detail.day?'Added to My Day':'Add to My Day';
            const wideFooter=iw>=440,footerHeight=wideFooter?64:112;
            v.place('detail-body',detailScroll,[left,144,iw,Math.max(120,h-144-footerHeight-32)]);
            if(status)v.label('status',status,[left,h-footerHeight-30,iw,24],{emphasized:true});
            else if(due.feedback.visible){due.feedback.setProperty('state',State.FLAT|(due.field.getProperty('invalid')?State.WARNING:0));v.place('date-feedback',due.feedback,[left,h-footerHeight-30,iw,24]);}
            const actionWidth=Math.min(iw,260),buttonWidth=Math.floor((actionWidth-8)/2);
            for(const slot of [saveSlot,cancelSlot])slot.setProperty('text',`panel:fixed:flush:width=${buttonWidth}:accessory-height=40`);
            v.place('details-actions',footerActions,[left+iw-actionWidth,h-footerHeight+8,actionWidth,40]);
            remove.text=pendingDelete?'Confirm delete':'Delete task';remove.setProperty('accessibleName',remove.text);
            v.place('delete-task',remove,[left,wideFooter?h-footerHeight+8:h-52,wideFooter?160:iw,40]);
        }
        if(status&&!detail)v.label('status',status,[rail+24,h-46,Math.max(160,w-rail-48),36],{emphasized:true});
    }});
    return view;
}
createTaskWorkspace.controlFactory=Object.freeze({options:['width','height','colors','storageKey','initialView'],usage:'For a To Do/task-manager request, import createTaskWorkspace and defineApp from @ricewm/app. export default defineApp({async start(context){return await createTaskWorkspace(context);}}). Manifest services: settings.get, settings.set; window1080x720. Native three-pane task workflow that follows the active theme, with real CheckBox completion and checklist steps and themed editors. Preserve the theme for reference-inspired composition. Override decorative surface colors only if explicitly requested. Options colors use exact background/panel/foreground/muted/accent/border #RRGGBB keys. initialView: my-day|important|planned|tasks. Native names quick-add/add-task/search/task-title/task-note/nav-tasks/close-details/save-details/cancel-details/delete-task; row names select-task-1/complete-task-1/important-task-1. Includes custom lists, sorting, due picker, steps, search, save/cancel, drafts, confirmation, persistent data, responsive inspector and keyboard quick add/navigation. User data starts empty. No Microsoft account, sync, share, email or timer integration. Use the generic Workspace for different workflows; this task component is optional.'});

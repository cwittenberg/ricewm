const smart=new Set(['my-day','important','planned','tasks']);
export const today=()=>{const d=new Date();return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;};
export const civilDate=value=>!value||/^\d{4}-\d{2}-\d{2}$/.test(value)&&Number.isFinite(Date.parse(value))&&new Date(value+'T12:00:00Z').toISOString().slice(0,10)===value;
export function taskStore(saved){
    if(saved==null)return {version:1,next:1,lists:[{id:'personal',title:'Personal'},{id:'work',title:'Work'}],tasks:[]};
    if(saved.version!==1||!Number.isSafeInteger(saved.next)||saved.next<1||!Array.isArray(saved.lists)||!Array.isArray(saved.tasks)||saved.lists.length>100||saved.tasks.length>1000)throw Error('Saved tasks are invalid. Your saved data has not been overwritten.');
    const ids=new Set();
    for(const l of saved.lists){if(!l||typeof l.id!=='string'||smart.has(l.id)||ids.has(l.id)||typeof l.title!=='string'||!l.title.trim()||l.title.length>120)throw Error('Invalid saved list');ids.add(l.id);}
    const tasks=new Set();for(const t of saved.tasks){if(!t||typeof t.id!=='string'||tasks.has(t.id)||!ids.has(t.list)||typeof t.title!=='string'||!t.title.trim()||t.title.length>300||typeof t.note!=='string'||t.note.length>16000||!civilDate(t.due)||typeof t.done!=='boolean'||typeof t.important!=='boolean'||typeof t.day!=='boolean'||!Array.isArray(t.steps)||t.steps.length>100||t.steps.some(s=>typeof s.title!=='string'||!s.title.trim()||typeof s.done!=='boolean'))throw Error('Invalid saved task');tasks.add(t.id);}
    return structuredClone(saved);
}
export function visibleTasks(store,list,query='',sort='created'){
    const q=query.trim().toLocaleLowerCase();
    const rows=store.tasks.filter(t=>(q||list==='tasks'||(list==='my-day'?t.day:list==='important'?t.important:list==='planned'?!!t.due:t.list===list))&&(!q||[t.title,t.note,...t.steps.map(s=>s.title)].join(' ').toLocaleLowerCase().includes(q)));
    return rows.sort((a,b)=>Number(a.done)-Number(b.done)||(sort==='title'?a.title.localeCompare(b.title):sort==='due'?(a.due||'9999').localeCompare(b.due||'9999'):0));
}
export function addTask(store,title,list){
    title=title.trim();if(!title)throw Error('Enter a task title.');if(title.length>300)throw Error('Task titles can have up to 300 characters.');if(store.tasks.length>=1000)throw Error('This workspace has reached 1,000 tasks.');
    const target=store.lists.some(l=>l.id===list)?list:store.lists[0]?.id;if(!target)throw Error('Create a list first.');
    const t={id:'task-'+store.next++,title,list:target,done:false,important:list==='important',day:list==='my-day',due:list==='planned'?today():'',note:'',steps:[]};store.tasks.push(t);return t;
}

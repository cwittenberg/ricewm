/** Per-window RiceBus endpoint. Only the shell can use an app's AI adapter. */
export function exposeAppCopilot({bus,instanceId,context,act,validate}){
    if(!bus||!instanceId)return ()=>{};
    return bus.expose(`${bus.owner}/copilot/${instanceId}`,async({source,request})=>{
        if(source!=='org.ricewm.shell')throw Error('AI app actions belong to the shell');
        const snapshot=await context(request?.query);
        if(request?.operation==='context')return snapshot;
        if(request?.operation==='capabilities')return {app:bus.owner,revision:snapshot.revision,ready:snapshot.ready,actions:snapshot.actions,queries:snapshot.queries||{}};
        if(!['act','validate'].includes(request?.operation)||snapshot.ready!==true||snapshot.revision!==request.expectedRevision)throw Error('The app changed or is busy. Refresh its AI context');
        if(!Object.hasOwn(snapshot.actions,request.action))throw Error('Unknown AI app action');
        if(request.operation==='validate'){if(typeof validate!=='function')throw Error('This app does not expose a command validator');return validate(request.action,request.data,request.expectedRevision);}
        return act(request.action,request.data,request.expectedRevision);
    });
}

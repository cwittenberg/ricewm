import {Button,Column,Label,Message,defineApp,desktopEventTopics} from '@ricewm/app';

export default defineApp({
    manifest:{id:'dev.herdr.generated.sdk-capabilities',name:'SDK desktop capabilities',version:'1.0.0'},
    start({app,bus,services}) {
        const status=new Label({text:'Registering desktop capabilities…'});
        const document=new Label({text:'Invoke an add-on from Paper, Sheet or Deck to inspect that document.'});
        const topics=desktopEventTopics(bus.owner);
        let invocation=null,notice=null,eventCount=0;
        const run=operation=>{void Promise.resolve().then(operation).catch(error=>{status.text=error.message;});};
        const button=(text,operation)=>{const control=new Button({text});control.on(Message.ACTIVATE,()=>run(operation));return control;};
        const stops=[
            bus.subscribe(topics.notificationAction,({payload})=>{status.text=`Notification action: ${payload.action}`;}),
            bus.subscribe(topics.hotkeyActivated,({payload})=>{status.text=`Hotkey ${payload.state}: ${payload.chord} (${++eventCount})`;}),
            bus.subscribe(topics.calendarChanged,()=>{status.text='Calendar changed. Refresh to see appointments.';}),
            bus.subscribe(topics.actionInvoked,({payload})=>run(async()=>{
                if(!payload.files.length){status.text='App action invoked';return;}
                const bytes=await services.files.read({handle:payload.files[0].handle});
                status.text=`Selected file: ${payload.files[0].name} · ${bytes.length} bytes · ${new TextDecoder().decode(bytes).slice(0,80)}`;
            })),
            bus.subscribe(topics.pluginInvoked,({payload})=>run(async()=>{
                invocation=payload;
                const context=await services.plugins.context({invocation:payload.invocation});
                document.text=`${payload.application}: ${payload.title} · revision ${context.revision} · ${context.text?.slice(0,80)||'Document command adapter ready'}`;
                status.text=`Add-on invoked: ${payload.application}`;
            })),
        ];
        app.on(Message.APP_CLOSE,()=>{for(const stop of stops)stop();});
        run(async()=>{
            await services.actions.register({id:'inspect-file',title:'Inspect with SDK capabilities',fileContext:'files'});
            for(const application of ['paper','sheet','deck'])await services.plugins.register({id:`inspect-${application}`,title:'Inspect document',application});
            await services.calendar.watch();
            status.text='Actions and document add-ons registered. Choose a capability below.';
        });
        return new Column({},[
            new Label({text:'SDK desktop capabilities'}),status,
            button('Show notification',async()=>{notice=await services.notifications.show({id:'demo',title:'Capability demo',message:'Use Open demo to send an app event.',action:'open-demo',actionLabel:'Open demo'});status.text='Notification shown';}),
            button('Dismiss notification',async()=>{if(notice)await services.notifications.dismiss(notice);status.text='Notification dismissed';}),
            button('Register Ctrl + Alt + P',async()=>{await services.hotkeys.register({id:'demo',chord:'Ctrl+Alt+KeyP'});status.text='Hotkey registered: Ctrl + Alt + P';}),
            button('Release hotkey',async()=>{await services.hotkeys.unregister({id:'demo'});status.text='Hotkey released';}),
            button('Read calendar',async()=>{
                const now=new Date(),start=[now.getFullYear(),String(now.getMonth()+1).padStart(2,'0'),String(now.getDate()).padStart(2,'0')].join('-');
                const result=await services.calendar.list({start,end:start});status.text=`Calendar: ${result.total} events today`;
            }),
            button('Choose a file',async()=>{
                const files=await services.dialogs.openFile({title:'Inspect a file'});if(!files?.length)return;
                const bytes=await services.files.read({handle:files[0].handle});status.text=`Chosen file: ${files[0].name} · ${bytes.length} bytes`;
            }),
            document,
            button('Append demo note to Paper',async()=>{
                if(invocation?.application!=='paper')throw Error('Invoke this add-on from a Paper window first.');
                const context=await services.plugins.context({invocation:invocation.invocation});
                const result=await services.plugins.apply({invocation:invocation.invocation,expectedRevision:context.revision,operation:'append',data:{text:'\nSDK add-on note'}});
                status.text=result.cancelled?'Document change cancelled':'Paper note appended';
            }),
        ]);
    },
});

import {Button,Column,Label,Message,defineApp,desktopEventTopics} from '@ricewm/app';

export default defineApp({
    manifest:{id:'dev.herdr.generated.notification-reminders',name:'Reminder SDK example',version:'1.0.0'},
    start({services,bus}) {
        const status=new Label({text:'RiceWM delivers reminders even after this app closes.'});
        bus.subscribe(desktopEventTopics(bus.owner).notificationAction,({payload})=>{status.text=`Notification action: ${payload.action}`;});
        const button=(text,operation)=>{
            const control=new Button({text});
            control.on(Message.ACTIVATE,()=>{void operation().catch(error=>{status.text=error.message;});});
            return control;
        };
        return new Column({},[
            new Label({text:'OS-managed reminders'}),status,
            button('Test action buttons',async()=>{
                await services.notifications.show({id:'choices',title:'Custom notification',message:'Choose an action.',duration:0,
                    actions:[{id:'open',label:'Open task'},{id:'done',label:'Mark done'}]});
            }),
            button('Schedule reminder',async()=>{
                await services.notifications.create({id:'task',title:'Task reminder',message:'Your task is due.',deliverAt:Date.now()+10_000,duration:0,actions:[{id:'snooze',label:'Snooze 10 seconds',snoozeMs:10_000},{id:'snooze-minute',label:'Snooze 1 minute',snoozeMs:60_000}]});
                status.text='Reminder scheduled in 10 seconds. You can close this app.';
            }),
            button('Edit and postpone',async()=>{
                await services.notifications.update({id:'task',title:'Updated reminder',message:'Your updated task is due.',deliverAt:Date.now()+15_000});
                status.text='Reminder updated and postponed to 15 seconds from now.';
            }),
            button('Delete reminder',async()=>{await services.notifications.delete({id:'task'});status.text='Reminder deleted.';}),
            button('List reminders',async()=>{
                const reminders=await services.notifications.list();
                status.text=reminders.length?reminders.map(record=>`${record.id}: ${record.state}`).join(' · '):'No reminders registered.';
            }),
        ]);
    },
});

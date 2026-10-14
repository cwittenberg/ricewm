import {defineApp, Button, Column, Label, Message} from '@ricewm/app';

export default defineApp({
    manifest: {id:'dev.ricewm.visual-recipes', name:'Visual recipes', version:'1.0.0'},
    start({services}) {
        const status = new Label({text:'Preview a quiet look, save it to Hub, then apply it.'});
        const recipe = {
            version:2, scope:'appearance', id:'quiet', title:'Quiet look',
            presentation:{
                'appearance.animations.window':'Glide',
                'appearance.animations.speed':'25',
                'appearance.windows.active_shadow':'false',
            },
        };
        let saved, run;
        const button = (text, action) => {
            const control = new Button({text});
            control.on(Message.ACTIVATE, async () => {
                try { await action(); }
                catch (error) { status.text = error.message; }
            });
            return control;
        };
        return new Column({}, [
            new Label({text:'Visual recipes use the same settings as Rice Hub.'}),
            status,
            button('Inspect presentation settings', async () => {
                const catalog = await services.recipes.presentation();
                status.text = `${catalog.settings.length} settings available. Window animation: ${catalog.values['appearance.animations.window']}`;
            }),
            button('Preview quiet look', async () => {
                const plan = await services.recipes.preview({recipe});
                status.text = plan.available ? 'Ready: Glide animation, speed 25, window shadows off. No changes applied.' : plan.desktop.reason;
            }),
            button('Save recipe to Hub', async () => {
                const existing = (await services.recipes.list()).find(record => record.recipe.id === recipe.id);
                saved = await services.recipes.save({recipe, revision:existing?.revision ?? null});
                status.text = 'Saved Quiet look. You can inspect and edit it in Hub.';
            }),
            button('Apply saved recipe', async () => {
                if (!saved) throw Error('Save the recipe first.');
                run = await services.recipes.apply({id:saved.recipe.id, revision:saved.revision});
                status.text = 'Recipe started. Refresh status to inspect the result.';
            }),
            button('Refresh status', async () => {
                if (!run) throw Error('Apply a saved recipe first.');
                const state = await services.recipes.status({id:run.id});
                status.text = state.report.failed?.message || `Recipe ${state.report.status}`;
            }),
        ]);
    },
});

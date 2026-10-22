import { Button, Column, Container, Label, Message, Row, ScrollView, State, TextEditor, TextField, defineApp } from '@ricewm/app';
import { issueForms, issueUrl } from './forms.mjs';

export default defineApp({
    start({ app, services, createWindow }) {
        const kind = app.manifest.id === 'org.ricewm.request-feature' ? 'feature' : 'bug';
        const form = issueForms[kind], fields = new Map();
        const summary = new TextField({ name: 'summary', accessibleName: 'Short summary', value: '' });
        fields.set('summary', summary);
        const body = [new Label({ text: form.description }), new Label({ text: 'Short summary *' }), summary];
        for (const field of form.fields) {
            const input = new TextEditor({ name: field.id, accessibleName: field.label, value: '', rows: field.rows });
            fields.set(field.id, input);
            body.push(new Label({ text: field.label + (field.required ? ' *' : '') }),
                new Container({ text: `panel:fixed-height:${field.rows * 20 + 16}` }, [input]));
        }
        body.push(new Label({ text: '* Required. You can attach screenshots and logs on GitHub.' }));
        const status = new Label({ name: 'status', text: '', visible: false });
        const report = new Button({ name: 'report-on-github', text: 'Report on our Github', state: State.ENABLED | State.ACCENT });
        report.on(Message.ACTIVATE, async () => {
            if (!report.enabled) return;
            status.visible = true;
            try {
                const url = issueUrl(kind, Object.fromEntries([...fields].map(([key, input]) => [key, input.getProperty('value')])));
                report.enabled = false;
                status.text = 'Opening GitHub…';
                await services.host.openExternal({ url });
                status.text = 'Review your report on GitHub, then submit it there. Your draft is still here.';
            } catch (error) {
                status.text = error.message || 'GitHub could not be opened. Please try again.';
            } finally {
                report.enabled = true;
            }
        });
        const cancel = new Button({ name: 'cancel', text: 'Close' });
        const window = createWindow({ title: form.name, root: new Column({ text: 'panel:padding=16:gap=12' }, [
            new ScrollView({ name: 'report-fields' }, [new Column({ text: 'panel:intrinsic-flow:tight-stack:gap=8' }, body)]),
            new Label({ text: 'GitHub reports are public. Leave out passwords and personal information.\nYou will need a GitHub account to submit.' }),
            status,
            new Row({ text: 'panel:intrinsic-flow:gap=8' }, [report, cancel]),
        ]) });
        cancel.on(Message.ACTIVATE, () => window.requestClose());
        return window;
    },
});

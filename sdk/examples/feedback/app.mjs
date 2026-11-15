import { Button, Column, Container, Label, Message, Row, ScrollView, State, TextEditor, TextField, defineApp } from '@ricewm/app';
import { issueForms, issueUrl } from './forms.mjs';

export default defineApp({
    start({ app, services, createWindow }) {
        const kind = app.manifest.id === 'org.ricewm.request-feature' ? 'feature' : 'bug';
        const form = issueForms[kind], fields = new Map();
        const flat = State.ENABLED | State.FLAT;
        const group = (label, input) => new Column({ text: 'panel:intrinsic-flow:flush:tight-stack:gap=4', state: flat }, [new Label({ text: label }), input]);
        const summary = new TextField({ name: 'summary', accessibleName: 'Short summary', value: '' });
        fields.set('summary', summary);
        const body = [new Label({ text: form.description }), group(kind === 'bug' ? 'Give it a short title' : 'Give your idea a short title', summary)];
        const optional = new Column({ name: 'optional-details', text: 'panel:intrinsic-flow:flush:tight-stack:gap=12', state: flat, visible: false });
        for (const field of form.fields) {
            const input = field.singleLine
                ? new TextField({ name: field.id, accessibleName: field.label, value: '' })
                : new TextEditor({ name: field.id, accessibleName: field.label, value: '', rows: field.rows });
            fields.set(field.id, input);
            const control = field.singleLine ? input : new Container({ text: `panel:fixed-height:${field.height}:flush`, state: flat }, [input]);
            const section = group(field.label, control);
            if (field.required) body.push(section);
            else optional.append(section);
        }
        const more = new Button({ name: 'more-details', text: 'Add more details (optional)', state: flat | State.START_ALIGNED });
        more.on(Message.ACTIVATE, async () => {
            optional.visible = !optional.visible;
            more.text = optional.visible ? 'Hide extra details' : 'Add more details (optional)';
            await window.fitContent();
            viewport.scrollTo(optional.visible ? optional : more);
        });
        body.push(more, optional);
        const viewport = new ScrollView({ name: 'report-fields', state: flat }, [new Column({ text: 'panel:intrinsic-flow:flush:tight-stack:gap=12', state: flat }, body)]);
        const status = new Label({ name: 'status', text: '', visible: false });
        const report = new Button({ name: 'report-on-github', text: 'Report on our GitHub', state: State.ENABLED | State.ACCENT });
        const hostReport = new Button({ name: 'report-on-github-host', text: 'Open in host browser' });
        const openReport = async (target = 'bowl') => {
            if (!report.enabled || !hostReport.enabled) return;
            status.visible = true;
            try {
                const values = Object.fromEntries([...fields].map(([key, input]) => [key, input.getProperty('value')]));
                const url = issueUrl(kind, values);
                report.enabled = false;
                hostReport.enabled = false;
                status.text = 'Opening GitHub…';
                await services.host.openExternal(target === 'host' ? { url, target } : { url });
                status.text = target === 'host'
                    ? 'Review and submit in your host browser. If no tab opened, allow pop-ups and try again. Your draft is still here.'
                    : 'Review your report on GitHub, then submit it there. Your draft is still here.';
            } catch (error) {
                status.text = error.message || 'GitHub could not be opened. Please try again.';
                const missing = ['summary', ...form.fields.filter(field => field.required).map(field => field.id)]
                    .find(id => !String(fields.get(id).getProperty('value') || '').trim());
                if (missing) {
                    viewport.scrollTo(fields.get(missing));
                    fields.get(missing).setProperty('focused', true);
                }
            } finally {
                report.enabled = true;
                hostReport.enabled = true;
                await window.fitContent();
            }
        };
        report.on(Message.ACTIVATE, () => openReport());
        hostReport.on(Message.ACTIVATE, () => openReport('host'));
        const cancel = new Button({ name: 'cancel', text: 'Close' });
        const window = createWindow({ title: form.name, fitContent: { width: 560, maxHeight: 680 }, root: new Column({ text: 'panel:padding=20:gap=12', state: State.ENABLED }, [
            viewport,
            new Label({ text: 'Review and submit on GitHub. You’ll need an account.\nReports are public. Please leave out private information.' }),
            status,
            new Row({ text: 'panel:intrinsic-flow:flush:gap=8', state: flat }, [report, hostReport, cancel]),
        ]) });
        app.on(Message.THEME_CHANGED, () => window.fitContent());
        cancel.on(Message.ACTIVATE, () => window.requestClose());
        return window;
    },
});

import { State } from './control.mjs';
import { Button, Column, Dialog, Image, Label, List, Markdown, Row, ScrollView, SplitPane } from './controls.mjs';
import { Message } from './messages.mjs';

/** Bowl's native About layout: a persistent hero and one sectioned document. */
export function createAppAboutDialog({ name, icon, tagline, sections, onClose }) {
    const documents = sections.map(([, text], index) => new Markdown({
        name: `about-document-${index}`, text, state: State.START_ALIGNED,
    }));
    const scroll = new ScrollView({ name: 'about-scroll', text: '0/0', state: State.EXPANSIVE }, [
        new Column({ name: 'about-content', text: 'panel:welcome-document' }, documents),
    ]);
    const navigation = sections.map(([text], index) => {
        const button = new Button({ name: `about-section-${index}`, text,
            icon: [icon, 78, 51, 52, 51][index] || 51,
            state: State.START_ALIGNED | (index === 0 ? State.SELECTED | State.ACCENT : State.FLAT),
        });
        button.on(Message.ACTIVATE, () => {
            scroll.text = `anchor:${documents[index].id}`;
            navigation.forEach((item, current) => item.setProperty('state', State.START_ALIGNED |
                (current === index ? State.SELECTED | State.ACCENT : State.FLAT)));
        });
        return button;
    });
    const close = new Button({ name: 'about-close', text: 'Close', icon: 54, accent: true });
    close.on(Message.ACTIVATE, onClose);
    const dialog = new Dialog({ name: 'about-dialog', text: `About ${name}`, icon,
        open: true, modal: true, state: State.EXPANSIVE }, [
        new SplitPane({ text: 'split:220:min-width=210:max-width=260' }, [
            new Column({ text: 'panel:welcome-navigation' }, [new List({ text: 'zoom:1150' }, navigation)]),
            new Column({ text: 'panel:clamp:760' }, [
                new Image({ name: 'about-icon', text: 'panel:height:120', icon, state: State.LARGE_ICON }),
                new Label({ text: name, state: State.DISPLAY | State.CENTER_ALIGNED }),
                new Label({ text: tagline, state: State.MUTED | State.CENTER_ALIGNED }),
                scroll,
                new Row({ text: 'panel:actions' }, [close]),
            ]),
        ]),
    ]);
    dialog.on(Message.KEY, event => {
        if (event.detail?.pressed && (event.detail.key === 'Escape' || event.detail.keyCode === 9)) {
            onClose(); event.preventDefault();
        }
    });
    return dialog;
}
createAppAboutDialog.controlFactory = Object.freeze({
    options: ['name', 'icon', 'tagline', 'sections', 'onClose'],
    guidance: 'Returns a native Dialog with section navigation. sections is an array of [title,markdown] pairs; onClose is the close callback. Attach the returned dialog to the application window and remove it on close.',
});

import { Column, DatePicker, Label, Message, Row, State, TimeSelector, defineApp } from '@ricewm/app';

export function buildDateTimeExample() {
    const date = new DatePicker({ name: 'appointment-date', label: 'Appointment date', locale: 'en-GB', expanded: true });
    const time = new TimeSelector({ name: 'appointment-time', label: 'Start time', hourCycle: 12, expanded: true });
    const summary = new Label({ text: 'Choose a date and time.', state: State.EMPHASIZED });
    const update = () => { summary.text = date.value && time.value ? `Your appointment: ${date.value} at ${time.display(time.value)}` : 'Choose a date and time.'; };
    for (const picker of [date, time]) picker.on(Message.VALUE_CHANGED, update);
    const root = new Column({ text: 'panel:fixed:gap=6' }, [
        new Row({ text: 'panel:fixed:flush:gap=20' }, [date, time]), summary,
    ]);
    return { root, date, time, summary };
}

export default defineApp({
    manifest: { id: 'dev.ricewm.date-time-example', name: 'Date & time', version: '1.0.0' },
    start({ createWindow }) { createWindow({ title: 'Date & time', width: 900, height: 620, root: buildDateTimeExample().root }); },
});

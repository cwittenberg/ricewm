export const issueForms = Object.freeze({
    bug: {
        name: 'Report bug', template: 'bug-report.yml', prefix: '[Bug] ',
        description: 'Something not working? Tell us what happened.',
        fields: [
            { id: 'happened', label: 'What went wrong?', required: true, rows: 3, height: 56 },
            { id: 'steps', label: 'How can we reproduce it?', required: true, rows: 3, height: 72 },
            { id: 'expected', label: 'What should have happened?', required: true, singleLine: true },
            { id: 'environment', label: 'Browser and operating system', required: true, singleLine: true },
            { id: 'extra', label: 'Anything else we should know?', rows: 3, height: 72 },
        ],
    },
    feature: {
        name: 'Request feature', template: 'feature-request.yml', prefix: '[Feature] ',
        description: 'Have an idea for RiceWM? We’d like to hear it.',
        fields: [
            { id: 'problem', label: 'What would you like to do?', required: true, rows: 3, height: 72 },
            { id: 'proposal', label: 'How would it work?', required: true, rows: 4, height: 88 },
            { id: 'alternatives', label: 'What have you tried instead?', rows: 3, height: 72 },
            { id: 'extra', label: 'Examples or other notes', rows: 3, height: 72 },
        ],
    },
});

export function issueUrl(kind, values) {
    const form = issueForms[kind];
    if (!form) throw new Error('Unknown report type');
    const summary = String(values.summary || '').trim();
    if (!summary) throw new Error('Please add a short summary.');
    if (Array.from(summary).length > 160) throw new Error('Please keep the summary to 160 characters.');
    if (/[\r\n\0]/u.test(summary)) throw new Error('Please keep the summary on one line.');
    const url = new URL('https://github.com/cwittenberg/ricewm/issues/new');
    url.searchParams.set('template', form.template);
    url.searchParams.set('title', form.prefix + summary);
    for (const field of form.fields) {
        const value = String(values[field.id] || '').trim();
        if (field.required && !value) throw new Error(`Please complete: ${field.label}.`);
        if (value) url.searchParams.set(field.id, value);
    }
    // GitHub rejects oversized request URLs. Keep every field intact and let
    // the author shorten the report rather than silently losing their text.
    if (url.href.length > 4000) throw new Error('This report is too long to open as a link. Shorten the details and attach longer logs on GitHub.');
    return url.href;
}

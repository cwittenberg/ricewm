export const issueForms = Object.freeze({
    bug: {
        name: 'Report bug', template: 'bug-report.yml', prefix: '[Bug] ',
        description: 'Tell us what went wrong and how to reproduce it.',
        fields: [
            { id: 'happened', label: 'What happened?', required: true, rows: 3 },
            { id: 'steps', label: 'Steps to reproduce', required: true, rows: 3 },
            { id: 'expected', label: 'What did you expect?', required: true, rows: 2 },
            { id: 'environment', label: 'RiceWM version, browser and operating system', required: true, rows: 2 },
            { id: 'extra', label: 'Anything else? (optional)', rows: 2 },
        ],
    },
    feature: {
        name: 'Request feature', template: 'feature-request.yml', prefix: '[Feature] ',
        description: 'Suggest something that would make RiceWM more useful to you.',
        fields: [
            { id: 'problem', label: 'What would this help you do?', required: true, rows: 3 },
            { id: 'proposal', label: 'How should it work?', required: true, rows: 3 },
            { id: 'alternatives', label: 'What have you tried instead? (optional)', rows: 2 },
            { id: 'extra', label: 'Examples or other details (optional)', rows: 2 },
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
    if (url.href.length > 7500) throw new Error('This report is too long to open as a link. Shorten the details and attach longer logs on GitHub.');
    return url.href;
}

// Native tables, clipboard text and Sheet files share the same readable cells.
export function tableText(value) {
    return String(value ?? '').replace(/[\t\r\n]/g, ' ').replace(/[\u0000-\u001f\u007f]/g, '');
}

export function seriesLabel(value, fallback) {
    // ECharts uses this private name when a raw-option series has no name.
    return /^series\u0000\d+$/.test(String(value ?? '')) || !value ? tableText(fallback) : tableText(value);
}

export function encodeTable(rows, format) {
    const values = [['Series', 'Time / Category', 'Value'], ...rows.map(row => [row.series, row.x, row.value])];
    return values.map(row => row.map(value => {
        const text = tableText(value);
        return format === 'csv' ? '"' + text.replaceAll('"', '""') + '"' : text;
    }).join(format === 'csv' ? ',' : '\t')).join(format === 'csv' ? '\r\n' : '\n');
}

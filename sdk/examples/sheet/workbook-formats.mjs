/** File naming and MIME metadata only; codecs remain inside the Wasm engine. */
export const WORKBOOK_FORMATS = Object.freeze({
    xlsx: Object.freeze({ mime: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', label: 'Excel workbook (.xlsx)' }),
    csv: Object.freeze({ mime: 'text/csv', label: 'Comma-separated values (.csv)' }),
    tsv: Object.freeze({ mime: 'text/tab-separated-values', label: 'Tab-separated values (.tsv)' }),
    ods: Object.freeze({ mime: 'application/vnd.oasis.opendocument.spreadsheet', label: 'OpenDocument spreadsheet (.ods)' }),
});
export function workbookFormat(name) {
    if (typeof name !== 'string' || !name || name.length > 255 || /[\p{Cc}\ud800-\udfff/\\]/u.test(name)) throw TypeError('Invalid workbook file name');
    const format = /\.(xlsx|ods|csv|tsv)$/iu.exec(name)?.[1].toLowerCase();
    if (!format) throw TypeError('Choose a workbook ending in .xlsx, .ods, .csv or .tsv; legacy .xls is not supported');
    return format;
}
export function workbookFileFilters(formats) {
    return formats.map(format => {
        const description = WORKBOOK_FORMATS[format];
        if (!description) throw TypeError('Unsupported workbook format');
        return description.mime;
    });
}

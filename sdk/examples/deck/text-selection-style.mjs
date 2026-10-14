const properties = ['fontFamily', 'fontSize', 'bold', 'italic', 'underline', 'color', 'align'];

/** Toolbar feedback is a projection of selected source styles, never an edit.
 * Missing properties remain unknown; mixed values are not guessed from run one. */
export function selectedTextStyle(objects) {
    const styles = objects.filter(object => object.text != null).flatMap(object => {
        // Synthetic paragraph separators carry paragraph defaults, not visible
        // glyph formatting. Match the native summary while retaining real
        // source soft breaks and genuinely different visible runs.
        const runs = object.text.runs?.filter(run => run.text.length > 0 && !(run.id === '' && run.text === '\n'));
        return runs?.length ? runs.map(run => run.style) : [object.text.style];
    });
    const mixed = [];
    const result = Object.fromEntries(properties.map(property => {
        const value = styles[0]?.[property] ?? null;
        if (styles.some(style => (style?.[property] ?? null) !== value)) {
            mixed.push(property); return [property, null];
        }
        return [property, value];
    }));
    return { ...result, mixed };
}

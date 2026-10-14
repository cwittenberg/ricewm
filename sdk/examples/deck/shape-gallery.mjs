import { Column, Label, Menu, MenuItem, Message, Row, ScrollView, State, Tooltip } from '@ricewm/app';
import { shapes, shapeById } from './shape-catalog.mjs';
import { SHAPE_ICONS } from './shape-icons.mjs';

const COLUMNS = 8;
/** Catalog-driven native popup. Shared semantic icons inherit desktop theme
 * contrast; RiceUI owns hover, focus, navigation and popup dismissal. */
export function createShapeGallery({ onSelect, notice = () => {} }) {
    let current = null;
    const content = new Column({ name: 'shape-gallery-categories', text: 'panel:fixed:gap=2:flush' });
    const scroll = new ScrollView({ name: 'shape-gallery-scroll', text: 'viewport:360|0/0', state: State.FLAT }, [content]);
    const control = new Menu({ name: 'insert-shapes', text: '', icon: 182, state: State.DISPLAY,
        accessibleName: 'Shapes', tooltip: 'Shapes', enabled: false }, [scroll]);
    function cell(shape) {
        const action = new MenuItem({ name: `shape-${shape.id}`,
            text: 'panel:fixed:accessory-height=36:width=36:flush', icon: SHAPE_ICONS[shape.id] || 182,
            accessibleName: shape.label, tooltip: shape.label }, [new Tooltip({ text: shape.label })]);
        action.on(Message.ACTIVATE, () => {
            if (!control.getProperty('enabled') || !shapeById(shape.id)) return;
            try { Promise.resolve(onSelect(shape.id)).catch(error => notice(error.message || 'Could not insert shape')); }
            catch (error) { notice(error.message || 'Could not insert shape'); }
        });
        return action;
    }
    function refresh() {
        const catalog = shapes();
        if (catalog === current) return;
        current = catalog;
        for (const child of [...content.children]) content.remove(child, { destroy: true });
        if (!catalog.length) { content.append(new Label({ text: 'Open a presentation to load shapes' })); return; }
        const categories = new Map();
        for (const shape of catalog) {
            if (!categories.has(shape.category)) categories.set(shape.category, []);
            categories.get(shape.category).push(shape);
        }
        let categoryIndex = 0;
        for (const [title, items] of categories) {
            content.append(new Label({ name: `shape-category-${categoryIndex++}`, text: title }));
            for (let start = 0; start < items.length; start += COLUMNS) {
                const row = new Row({ text: 'panel:fixed:gap=0:flush' });
                for (const shape of items.slice(start, start + COLUMNS)) row.append(cell(shape));
                content.append(row);
            }
        }
    }
    refresh();
    return Object.freeze({ control, refresh, setEnabled(enabled) { control.setProperty('enabled', enabled); } });
}

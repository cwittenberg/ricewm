import {CONTROL_CLASSES as primitives,ui as primitiveUi} from './controls.mjs';
import {Chart} from './charts.mjs';
import {Table,DataTable} from './data-table.mjs';

/** Public native UI factories include the retained chart composite. */
export const CONTROL_CLASSES=Object.freeze({...primitives,Chart,Table,DataTable});
export function createControl(type,properties={},children=[]) {
    const Constructor=CONTROL_CLASSES[type];
    if(!Constructor)throw TypeError(`unknown RiceUI control ${type}`);
    if(type==='Chart'&&children.length)throw TypeError('Chart owns its native children; supply data/spec or option');
    return new Constructor(properties,children);
}
export const ui=Object.freeze({...primitiveUi,chart:(properties={})=>new Chart(properties),table:(properties={},children=[])=>new Table(properties,children),dataTable:(properties={})=>new DataTable(properties)});

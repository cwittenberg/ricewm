export { checkedSelectionValidation } from "./validation-contract.mjs";
import { checkedDataToolOptions } from "./data-tools-contract.mjs";
import { checkedChart, checkedChartBounds } from "./chart-contract.mjs";
import { checkedViewport } from "./viewport-contract.mjs";
import { checkedConditionalRule } from './conditional-format-contract.mjs';
import { checkedDataValidation } from './validation-authoring-contract.mjs';
import { checkedPasteOptions } from './paste-contract.mjs';
import {checkedSheetFindOptions,checkedSheetFindTarget,checkedSheetReplaceRequest} from './find-contract.mjs';
import {checkedSheetSpecialCellsRequest} from './special-cells-contract.mjs';
/** Semantic commands use zero-based logical worksheet coordinates, never visible
 * row slots. A backend adapting a one-based engine must translate at its boundary.
 * Cell input is uninterpreted text/formula; calculation belongs to the engine.
 */
export const SHEET_COMMANDS = Object.freeze([
  'create-table',
  'rename-table',
  'paste-text',
  'paste-special',
  'set-data-validation',
  'clear-data-validations',
  'freeze-panes',
  'add-conditional-format',
  'update-conditional-format',
  'delete-conditional-format',
  'move-conditional-format',
  'clear-conditional-formats',
  "select-validation-value",
  "create-defined-name",
  "update-defined-name",
  "delete-defined-name",
  "select-sheet",
  "select-cell",
  "select-range",
  "select-ranges",
  'select-special-cells',
  "traverse-selection",
  "fill-selection",
  "modify-selection",
  "navigate-range",
  "navigate-reference",
  "find-next",
  "replace-text",
  "edit-cell",
  "merge-cells",
  "unmerge-cells",
  "fill-range",
  "copy-range",
  'cut-range', 'cancel-clipboard',
  "paste-range",
  "clear-range",
  "clear-ranges",
  "format-range",
  "format-cells",
  "format-ranges",
  "paint-format",
  "apply-cell-style",
  "delete-cell-style",
  "undo",
  "redo",
  "create-sheet",
  "rename-sheet",
  "delete-sheet",
  "duplicate-sheet",
  "move-sheet",
  "insert-rows",
  "delete-rows",
  "insert-columns",
  "delete-columns",
  "sort-range",
  "remove-duplicates",
  "text-to-columns",
  "resize-rows",
  "resize-columns",
  "toggle-autofilter",
  "fill-down-adjacent",
  "create-chart",
  "update-chart",
  "delete-chart",
  "move-chart",
]);
/** Feature advertisements share this canonical catalog, but are not commands. */
export const SHEET_FEATURE_CAPABILITIES=Object.freeze(['table-metadata','data-validation','find-results','special-cells','special-cell-metadata','special-cell-geometry','special-cell-differences','special-cell-references']);
export const SHEET_CAPABILITIES=Object.freeze([...SHEET_COMMANDS,...SHEET_FEATURE_CAPABILITIES]);
export const SHEET_INPUT_MAX_BYTES = 16 * 1024;
export const SHEET_PASTE_TEXT_MAX_BYTES=24*1024;
/** Persistent document effects, shared by save-state tracking and automation.
 * Selection and internal copying do not author workbook content. New editing
 * commands are persistent by default, preventing silent save-state omissions. */
export function sheetCommandEffect(type){
  if(!SHEET_COMMANDS.includes(type))throw Object.assign(new TypeError('Unsupported Sheet command'),{code:'UNSUPPORTED_COMMAND'});
  return ['select-sheet','select-cell','select-range','select-ranges','select-special-cells','modify-selection','traverse-selection','navigate-range','navigate-reference','find-next','copy-range','cut-range','cancel-clipboard'].includes(type)?'query':'mutation';
}
const fail = (message) => {
  throw Object.assign(new TypeError(message), { code: "INVALID_COMMAND" });
};
// Match the engine's finite decimal point profile without rounding the value.
const fontPoints = (value) =>
  typeof value === "string" &&
  value.length <= 64 &&
  /^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/.test(value) &&
  Number.isFinite(Number(value)) &&
  Number(value) >= 1 &&
  Number(value) <= 409;

const exact = (value, keys) =>
  value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).length === keys.length &&
  keys.every((key) => Object.hasOwn(value, key));
const sheet = (value) =>
  typeof value === "string" && value.length > 0 && value.length <= 128;
const coordinate = (value, maximum) =>
  Number.isSafeInteger(value) && value >= 0 && value < maximum;
const cells = (range) =>
  (range.lastRow - range.firstRow + 1) *
  (range.lastColumn - range.firstColumn + 1);

export function checkedSheetRange(value) {
  if (
    !exact(value, ["firstRow", "lastRow", "firstColumn", "lastColumn"]) ||
    !coordinate(value.firstRow, 1048576) ||
    !coordinate(value.lastRow, 1048576) ||
    !coordinate(value.firstColumn, 16384) ||
    !coordinate(value.lastColumn, 16384) ||
    value.firstRow > value.lastRow ||
    value.firstColumn > value.lastColumn
  )
    fail("Invalid logical Sheet range");
  return Object.freeze({
    firstRow: value.firstRow,
    lastRow: value.lastRow,
    firstColumn: value.firstColumn,
    lastColumn: value.lastColumn,
  });
}
/** Internal clipboard identity contains no cell contents. Missing metadata is
 * compatible with existing backends that do not advertise the Cut lifecycle. */
export function checkedSheetClipboard(value){
  if(value===undefined||value===null)return null;
  if(!exact(value,['mode','generation','sheetId','range'])||!['copy','cut'].includes(value.mode)||!sheet(value.sheetId)
    ||!Number.isSafeInteger(value.generation)||value.generation<1)fail('Invalid Sheet clipboard metadata');
  const range=checkedSheetRange(value.range);
  if(cells(range)>(value.mode==='copy'?4096:10000))fail('Clipboard range exceeds the engine operation limit');
  return Object.freeze({mode:value.mode,generation:value.generation,sheetId:value.sheetId,range});
}
function clipboardFields(value){
  if(!Object.hasOwn(value,'clipboardGeneration'))return {};
  if(!Number.isSafeInteger(value.clipboardGeneration)||value.clipboardGeneration<1)fail('Invalid clipboard generation');
  return {clipboardGeneration:value.clipboardGeneration};
}

export function checkedCapabilities(value = []) {
  if (
    !Array.isArray(value) ||
    value.length > SHEET_CAPABILITIES.length ||
    new Set(value).size !== value.length ||
    value.some(
      (name) => !SHEET_CAPABILITIES.includes(name),
    )
  )
    fail("Invalid Sheet command capabilities");
  if(value.includes('find-results')&&!value.includes('find-next'))fail('Find result capability requires native Find');
  return Object.freeze([...value]);
}

export function checkedSheetDisjointRanges(ranges){
  if(!Array.isArray(ranges)||!ranges.length||ranges.length>512)fail('Invalid multi-area ranges');
  const checked=ranges.map(checkedSheetRange);
  for(let i=0;i<checked.length;i++){
    const a=checked[i];if(checked.slice(0,i).some(b=>a.firstRow<=b.lastRow&&b.firstRow<=a.lastRow&&a.firstColumn<=b.lastColumn&&b.firstColumn<=a.lastColumn))fail('Selection areas must be disjoint');
  }
  return Object.freeze(checked);
}
export function checkedSheetSelectionAreas(ranges,active){
  if(!exact(active,['row','column'])||!coordinate(active.row,1048576)||!coordinate(active.column,16384))fail('Invalid multi-area selection');
  const checked=checkedSheetDisjointRanges(ranges);
  if(!checked.some(a=>a.firstRow<=active.row&&active.row<=a.lastRow&&a.firstColumn<=active.column&&active.column<=a.lastColumn))fail('The active cell must belong to the selection');
  return Object.freeze({ranges:checked,active:Object.freeze({...active})});
}
export function checkedSheetCommand(value) {
  const type = value?.type;
  if (!SHEET_COMMANDS.includes(type))
    throw Object.assign(new TypeError("Unsupported Sheet command"), {
      code: "UNSUPPORTED_COMMAND",
    });
  if(type==='find-next'){
    if(!exact(value,['type','options','after','direction'])||!['forward','backward'].includes(value.direction))fail('Invalid Find command');
    return Object.freeze({type,options:checkedSheetFindOptions(value.options),after:checkedSheetFindTarget(value.after),direction:value.direction});
  }
  if(type==='select-ranges'){
    if(!exact(value,['type','sheetId','ranges','active'])||!sheet(value.sheetId))fail('Invalid multi-area selection');
    return Object.freeze({type,sheetId:value.sheetId,...checkedSheetSelectionAreas(value.ranges,value.active)});
  }
  if(type==='select-special-cells'){
    if(!exact(value,['type','sheetId','ranges','kind','types']))fail('Invalid Go To Special selection');
    const {type:_,...fields}=value;return Object.freeze({type,...checkedSheetSpecialCellsRequest(fields)});
  }
  if(type==='modify-selection'){
    if(!exact(value,['type','sheetId','range','anchor','unit','operation'])||!sheet(value.sheetId)||!['cell','row','column','sheet'].includes(value.unit)||!['add','remove','toggle'].includes(value.operation))fail('Invalid selection gesture');
    const range=checkedSheetRange(value.range);const {active:anchor}=checkedSheetSelectionAreas([range],value.anchor);
    if(value.unit==='row'&&(range.firstColumn!==0||range.lastColumn!==16383)||value.unit==='column'&&(range.firstRow!==0||range.lastRow!==1048575)||value.unit==='sheet'&&(range.firstRow!==0||range.lastRow!==1048575||range.firstColumn!==0||range.lastColumn!==16383))fail('Selection gesture unit does not match its range');
    return Object.freeze({type,sheetId:value.sheetId,range,anchor,unit:value.unit,operation:value.operation});
  }
  if(type==='fill-selection'){
    const warning=Object.hasOwn(value,'allowValidationWarning');
    if(!exact(value,['type','sheetId','ranges','origin','input',...(warning?['allowValidationWarning']:[])])||!sheet(value.sheetId))fail('Invalid selection input');
    const {ranges,active:origin}=checkedSheetSelectionAreas(value.ranges,value.origin);
    if(ranges.reduce((sum,r)=>sum+cells(r),0)>10000||typeof value.input!=='string'||/[\ud800-\udfff\u0000]/u.test(value.input)||new TextEncoder().encode(value.input).length>SHEET_INPUT_MAX_BYTES||new TextEncoder().encode(JSON.stringify(value.input)).length>24576||warning&&typeof value.allowValidationWarning!=='boolean')fail('Selection input exceeds native text or cell limits');
    return Object.freeze({type,sheetId:value.sheetId,ranges,origin,input:value.input,...(warning?{allowValidationWarning:value.allowValidationWarning}:{})});
  }
  if(type==='traverse-selection'){
    if(!exact(value,['type','sheetId','order','direction'])||!sheet(value.sheetId)||!['rows','columns'].includes(value.order)||!['forward','backward'].includes(value.direction))fail('Invalid selection traversal');
    return Object.freeze({type,sheetId:value.sheetId,order:value.order,direction:value.direction});
  }
  if(type==='clear-ranges'){
    if(!exact(value,['type','sheetId','ranges'])||!sheet(value.sheetId))fail('Invalid multi-area clear');
    const ranges=checkedSheetDisjointRanges(value.ranges);
    if(ranges.reduce((sum,r)=>sum+cells(r),0)>10000)fail('Clear exceeds 10000 selected cells');
    return Object.freeze({type,sheetId:value.sheetId,ranges});
  }
  if(type==='replace-text'){
    if(!exact(value,['type','options','replacement','target']))fail('Invalid Replace command');
    const {type:_,...request}=value;return Object.freeze({type,...checkedSheetReplaceRequest(request)});
  }
  if(type==='paste-text'){
    if(!exact(value,['type','sheetId','target','text','mode'])||!sheet(value.sheetId)||typeof value.text!=='string'
      ||!['all','values'].includes(value.mode)||new TextEncoder().encode(value.text).length>SHEET_PASTE_TEXT_MAX_BYTES
      ||/[\ud800-\udfff\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/u.test(value.text))fail('Invalid clipboard text paste');
    const target=checkedSheetRange(value.target);if(cells(target)>10000)fail('Paste text destination exceeds the engine operation limit');
    return Object.freeze({type,sheetId:value.sheetId,target,text:value.text,mode:value.mode});
  }
  if(type==='paste-special'){
    const clipboard=clipboardFields(value);
    if(!exact(value,['type','sheetId','target','options',...Object.keys(clipboard)])||!sheet(value.sheetId))fail('Invalid Paste Special command fields');
    const target=checkedSheetRange(value.target);if(cells(target)>10000)fail('Paste Special target exceeds the engine operation limit');
    return Object.freeze({type,sheetId:value.sheetId,target,options:checkedPasteOptions(value.options),...clipboard});
  }
  if(type==='cancel-clipboard'){
    const clipboard=clipboardFields(value);if(!exact(value,['type',...Object.keys(clipboard)]))fail('Invalid Cancel Clipboard fields');
    return Object.freeze({type,...clipboard});
  }
  if(type==='set-data-validation'||type==='clear-data-validations'){
    const setting=type==='set-data-validation';
    if(!exact(value,['type','sheetId','ranges',...(setting?['rule']:[])])||!sheet(value.sheetId))fail('Invalid validation command fields');
    let ranges=null;
    if(value.ranges!==null||setting){
      if(!Array.isArray(value.ranges)||!value.ranges.length||value.ranges.length>64)fail('Validation requires 1–64 logical ranges');
      ranges=Object.freeze(value.ranges.map(checkedSheetRange));
    }
    return Object.freeze({...value,ranges,...(setting?{rule:checkedDataValidation(value.rule)}:{})});
  }
  if(type==='freeze-panes'){
    if(!exact(value,['type','sheetId','rows','columns'])||!sheet(value.sheetId)||!coordinate(value.rows,1048576)||!coordinate(value.columns,16384))fail('Invalid frozen pane counts');
    return Object.freeze({...value});
  }
  if(type==='rename-table'){
    const optional=Object.hasOwn(value,'allowValidationWarning')?['allowValidationWarning']:[];
    const named=n=>typeof n==='string'&&n.length>0&&[...n].length<=255&&!/[\p{Cc}\p{Cs}]/u.test(n);
    const heading=n=>typeof n==='string'&&n.length>0&&[...n].length<=255&&!/[\p{Cs}]/u.test(n)&&!/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F]/u.test(n);
    if(!exact(value,['type','table','name','columns',...optional])||!named(value.table)||value.name!==null&&!named(value.name)
      ||!Array.isArray(value.columns)||value.columns.length>16384||optional.length&&typeof value.allowValidationWarning!=='boolean')fail('Invalid table rename fields');
    const ids=new Set();let bytes=new TextEncoder().encode(value.table+(value.name??'')).length;
    const columns=Object.freeze(value.columns.map(c=>{
      if(!exact(c,['id','name'])||!Number.isSafeInteger(c.id)||c.id<1||c.id>0xffffffff||ids.has(c.id)||!heading(c.name))fail('Invalid table column rename');
      ids.add(c.id);bytes+=new TextEncoder().encode(c.name).length;return Object.freeze({...c});
    }));
    if(bytes>65536)fail('Table rename text exceeds 64 KiB');
    return Object.freeze({...value,columns});
  }
  if(type==='create-table'){
    if(!exact(value,['type','sheetId','range','name','hasHeaders'])||!sheet(value.sheetId)||typeof value.hasHeaders!=='boolean'
      ||value.name!==null&&(typeof value.name!=='string'||!value.name.length||[...value.name].length>255||/[\p{Cc}\p{Cs}]/u.test(value.name)))fail('Invalid table creation fields');
    const range=checkedSheetRange(value.range);if(cells(range)>10000)fail('Table creation exceeds the current operation limit');
    return Object.freeze({...value,range});
  }
  if (['add-conditional-format','update-conditional-format','delete-conditional-format','move-conditional-format','clear-conditional-formats'].includes(type)) {
    const authored=['add-conditional-format','update-conditional-format'].includes(type);
    const indexed=['update-conditional-format','delete-conditional-format','move-conditional-format'].includes(type);
    const ranged=authored||type==='clear-conditional-formats';
    if(!exact(value,['type','sheetId',...(indexed?['ruleIndex']:[]),...(ranged?['ranges']:[]),...(authored?['rule']:[]),...(type==='move-conditional-format'?['direction']:[])])||!sheet(value.sheetId))fail('Invalid conditional-format command fields');
    if(indexed&&!coordinate(value.ruleIndex,4096))fail('Invalid conditional-format rule identity');
    if(type==='move-conditional-format'&&!['up','down'].includes(value.direction))fail('Invalid conditional-format priority direction');
    const result={...value};
    if(ranged){
      if(value.ranges===null&&type==='clear-conditional-formats')result.ranges=null;
      else{
        if(!Array.isArray(value.ranges)||!value.ranges.length||value.ranges.length>64)fail('Conditional formatting requires 1–64 logical ranges');
        result.ranges=Object.freeze(value.ranges.map(checkedSheetRange));
      }
    }
    if(authored)result.rule=checkedConditionalRule(value.rule);
    return Object.freeze(result);
  }
  if (type === "select-validation-value") {
    if (
      !exact(value, [
        "type",
        "sheetId",
        "row",
        "column",
        "ruleIndex",
        "itemIndex",
      ]) ||
      !sheet(value.sheetId) ||
      !coordinate(value.row, 1048576) ||
      !coordinate(value.column, 16384) ||
      !coordinate(value.ruleIndex, 1024) ||
      !coordinate(value.itemIndex, 10000)
    )
      fail("Invalid validation list choice");
    return Object.freeze({ ...value });
  }
  if (
    [
      "create-defined-name",
      "update-defined-name",
      "delete-defined-name",
    ].includes(type)
  ) {
    const keys = [
      "type",
      "name",
      "scopeSheetId",
      ...(type === "delete-defined-name" ? [] : ["formula"]),
      ...(type === "update-defined-name" ? ["newName", "newScopeSheetId"] : []),
    ];
    const name = (word) =>
      typeof word === "string" &&
      [...word].length >= 1 &&
      [...word].length <= 255 &&
      !/[\p{Cc}\ud800-\udfff]/u.test(word);
    const scope = (id) => id === null || sheet(id);
    if (
      !exact(value, keys) ||
      !name(value.name) ||
      !scope(value.scopeSheetId) ||
      (type !== "delete-defined-name" &&
        (typeof value.formula !== "string" ||
          !value.formula.length ||
          new TextEncoder().encode(value.formula).length > 16384 ||
          /[\u0000\ud800-\udfff]/u.test(value.formula))) ||
      (type === "update-defined-name" &&
        (!name(value.newName) || !scope(value.newScopeSheetId)))
    )
      fail("Invalid defined-name command");
    return Object.freeze({ ...value });
  }
  if (["remove-duplicates", "text-to-columns"].includes(type)) {
    const { type: _, ...options } = value;
    return Object.freeze({
      type,
      ...checkedDataToolOptions(options, type, { fail }),
    });
  }
  if (type === "create-chart" || type === "update-chart") {
    if (!exact(value, ["type", "chart"])) fail("Invalid chart command");
    return Object.freeze({
      type,
      chart: checkedChart(value.chart, { draft: type === "create-chart" }),
    });
  }
  if (type === "delete-chart" || type === "move-chart") {
    if (
      !exact(value, [
        "type",
        "id",
        ...(type === "move-chart"
          ? ["chartRevision", "viewport", "bounds"]
          : []),
      ]) ||
      !Number.isSafeInteger(value.id) ||
      value.id < 1 ||
      value.id > 0xffffffff
    )
      fail("Invalid chart identity");
    if (type === "delete-chart") return Object.freeze({ type, id: value.id });
    if (!Number.isSafeInteger(value.chartRevision) || value.chartRevision < 0)
      fail("Invalid chart revision");
    return Object.freeze({
      type,
      id: value.id,
      chartRevision: value.chartRevision,
      viewport: checkedViewport(value.viewport),
      bounds: checkedChartBounds(value.bounds),
    });
  }
  if (type === "undo" || type === "redo" || type === "create-sheet") {
    if (!exact(value, ["type"])) fail("Command cannot contain cell arguments");
    return Object.freeze({ type });
  }
  if (type === "rename-sheet") {
    if (
      !exact(value, ["type", "sheetId", "name"]) ||
      !sheet(value.sheetId) ||
      typeof value.name !== "string" ||
      !value.name.length ||
      [...value.name].length > 31 ||
      /[\p{Cc}\ud800-\udfff\[\]:*?/\\]/u.test(value.name) ||
      value.name.startsWith("'") ||
      value.name.endsWith("'")
    )
      fail(
        "Sheet names need 1–31 characters and cannot contain controls, []:*?/\\ or leading/trailing apostrophes.",
      );
    return Object.freeze({ type, sheetId: value.sheetId, name: value.name });
  }
  if (type === "move-sheet") {
    if (
      !exact(value, ["type", "sheetId", "index"]) ||
      !sheet(value.sheetId) ||
      !coordinate(value.index, 256)
    )
      fail("Invalid sheet tab position");
    return Object.freeze({ type, sheetId: value.sheetId, index: value.index });
  }
  if (type === "resize-rows" || type === "resize-columns") {
    const rows = type === "resize-rows",
      first = rows ? "firstRow" : "firstColumn",
      last = rows ? "lastRow" : "lastColumn";
    const dimension = rows ? "heightPt" : "widthPx",
      maximum = rows ? 1048576 : 16384;
    if (
      !exact(value, ["type", "sheetId", first, last, dimension]) ||
      !sheet(value.sheetId) ||
      !coordinate(value[first], maximum) ||
      !coordinate(value[last], maximum) ||
      value[last] < value[first] ||
      value[last] - value[first] >= 4096 ||
      !Number.isFinite(value[dimension]) ||
      value[dimension] < 1 ||
      value[dimension] > (rows ? 409 : 4096)
    )
      fail(
        "Resize requires 1–4096 tracks and a positive size within the supported bounds",
      );
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      [first]: value[first],
      [last]: value[last],
      [dimension]: value[dimension],
    });
  }
  if (
    ["insert-rows", "delete-rows", "insert-columns", "delete-columns"].includes(
      type,
    )
  ) {
    const maximum = type.endsWith("rows") ? 1048576 : 16384;
    if (
      !exact(value, ["type", "sheetId", "index", "count"]) ||
      !sheet(value.sheetId) ||
      !coordinate(value.index, maximum) ||
      !Number.isSafeInteger(value.count) ||
      value.count < 1 ||
      value.count > 4096 ||
      value.index + value.count > maximum
    )
      fail("Select between 1 and 4096 rows or columns within worksheet bounds");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      index: value.index,
      count: value.count,
    });
  }
  if (["select-sheet", "delete-sheet", "duplicate-sheet"].includes(type)) {
    if (!exact(value, ["type", "sheetId"]) || !sheet(value.sheetId))
      fail("Invalid Sheet selection command");
    return Object.freeze({ type, sheetId: value.sheetId });
  }
  if (type === "toggle-autofilter" || type === "fill-down-adjacent") {
    const hasRange = value.range !== undefined;
    if (
      !exact(value, ["type", "sheetId", ...(hasRange ? ["range"] : [])]) ||
      !sheet(value.sheetId) ||
      (type === "fill-down-adjacent" && !hasRange)
    )
      fail("Invalid worksheet range command");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      ...(hasRange ? { range: checkedSheetRange(value.range) } : {}),
    });
  }
  if(type==='navigate-reference'){
    if(!exact(value,['type','sheetId','reference'])||!sheet(value.sheetId)||typeof value.reference!=='string'||!value.reference.trim()||new TextEncoder().encode(value.reference).length>8192||/[\p{Cc}\ud800-\udfff]/u.test(value.reference))fail('Invalid navigation reference');
    return Object.freeze({type,sheetId:value.sheetId,reference:value.reference});
  }
  if (type === "select-range" || type==='navigate-range') {
    if (
      !exact(value, [
        "type",
        "sheetId",
        "firstRow",
        "lastRow",
        "firstColumn",
        "lastColumn",
      ]) ||
      !sheet(value.sheetId)
    )
      fail("Invalid range selection");
    const { type: _, sheetId, ...range } = value;
    return Object.freeze({ type, sheetId, ...checkedSheetRange(range) });
  }
  if (type === "sort-range") {
    if (
      !exact(value, [
        "type",
        "sheetId",
        "range",
        "header",
        "keys",
        "caseSensitive",
      ]) ||
      !sheet(value.sheetId) ||
      typeof value.header !== "boolean" ||
      typeof value.caseSensitive !== "boolean" ||
      !Array.isArray(value.keys) ||
      value.keys.length < 1 ||
      value.keys.length > 8
    )
      fail("Sort needs a range, header option and 1–8 column keys");
    const range = checkedSheetRange(value.range);
    if (
      cells(range) > 10000 ||
      range.lastRow - range.firstRow + 1 <= Number(value.header)
    )
      fail("Sort range needs data rows and at most 10000 cells");
    const seen = new Set(),
      keys = value.keys.map((key) => {
        if (
          !exact(key, ["column", "direction"]) ||
          !coordinate(key.column, 16384) ||
          key.column < range.firstColumn ||
          key.column > range.lastColumn ||
          !["ascending", "descending"].includes(key.direction) ||
          seen.has(key.column)
        )
          fail("Choose distinct sort columns inside the range");
        seen.add(key.column);
        return Object.freeze({ column: key.column, direction: key.direction });
      });
    if (seen.size !== keys.length)
      fail("Sort keys must not contain missing entries");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      range,
      header: value.header,
      keys: Object.freeze(keys),
      caseSensitive: value.caseSensitive,
    });
  }
  if (type === "fill-range") {
    if (
      !exact(value, ["type", "sheetId", "source", "target"]) ||
      !sheet(value.sheetId)
    )
      fail("Invalid range fill");
    const source = checkedSheetRange(value.source),
      target = checkedSheetRange(value.target);
    if (
      cells(target) > 10000 ||
      target.firstRow > source.firstRow ||
      target.lastRow < source.lastRow ||
      target.firstColumn > source.firstColumn ||
      target.lastColumn < source.lastColumn ||
      !(
        (target.firstRow === source.firstRow &&
          target.lastRow === source.lastRow) ||
        (target.firstColumn === source.firstColumn &&
          target.lastColumn === source.lastColumn)
      )
    )
      fail("Fill must extend the source along one axis within 10000 cells");
    return Object.freeze({ type, sheetId: value.sheetId, source, target });
  }
  if (type === "delete-cell-style") {
    if (
      !exact(value, ["type", "name"]) ||
      typeof value.name !== "string" ||
      !value.name.length ||
      new TextEncoder().encode(value.name).length > 128 ||
      /[\p{Cc}\ud800-\udfff]/u.test(value.name)
    )
      fail("Invalid cell style");
    return Object.freeze({ type, name: value.name });
  }
  if (type === "apply-cell-style") {
    if (
      !exact(value, ["type", "sheetId", "range", "name"]) ||
      !sheet(value.sheetId) ||
      typeof value.name !== "string" ||
      !value.name.length ||
      new TextEncoder().encode(value.name).length > 128 ||
      /[\p{Cc}\ud800-\udfff]/u.test(value.name)
    )
      fail("Invalid cell style");
    const range = checkedSheetRange(value.range);
    if (cells(range) > 10000) fail("Style range exceeds 10000 cells");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      range,
      name: value.name,
    });
  }
  if (type === "format-cells" || type==='format-ranges') {
    const multiple=type==='format-ranges';
    const optional = ["changes", "borders"].filter((key) =>
      Object.hasOwn(value, key),
    );
    if (
      !exact(value, ["type", "sheetId", multiple?'ranges':'range', ...optional]) ||
      !sheet(value.sheetId)
    )
      fail("Invalid cell format command");
    const ranges=multiple?checkedSheetDisjointRanges(value.ranges):[checkedSheetRange(value.range)],
      changes = value.changes || [],
      borders = value.borders || [];
    if (
      ranges.reduce((sum,r)=>sum+cells(r),0) > 10000 ||
      !Array.isArray(changes) ||
      !Array.isArray(borders) ||
      changes.length > 16 ||
      borders.length > 10 ||
      (!changes.length && !borders.length)
    )
      fail("Invalid format transaction bounds");
    const seen = new Set(),
      checkedChanges = changes.map((change) => {
        if (
          !exact(change, ["path", "value"]) ||
          seen.has(change.path) ||
          typeof change.value !== "string" ||
          new TextEncoder().encode(change.value).length > 128 ||
          /[\p{Cc}\ud800-\udfff]/u.test(change.value)
        )
          fail("Invalid format property");
        seen.add(change.path);
        const v = change.value;
        const valid = [
          "font.b",
          "font.i",
          "font.u",
          "font.strike",
          "alignment.wrap_text",
        ].includes(change.path)
          ? ["true", "false"].includes(v)
          : change.path === "font.size"
            ? fontPoints(v)
            : ["font.color", "fill.color"].includes(change.path)
              ? v === "none" || /^#[0-9a-f]{6}$/i.test(v)
              : change.path === "alignment.horizontal"
                ? ["general", "left", "center", "right"].includes(v)
                : change.path === "alignment.vertical"
                  ? ["top", "center", "bottom"].includes(v)
                  : ["font.name", "num_fmt"].includes(change.path) &&
                    Boolean(v.trim());
        if (!valid) fail("Unsupported cell format");
        return Object.freeze({ ...change });
      });
    const checkedBorders = borders.map((border) => {
      if (
        !exact(border, ["edge", "style", "color"]) ||
        ![
          "all",
          "none",
          "outline",
          "inside",
          "top",
          "right",
          "bottom",
          "left",
          "inside-horizontal",
          "inside-vertical",
        ].includes(border.edge) ||
        ![
          "none",
          "thin",
          "medium",
          "thick",
          "double",
          "dotted",
          "mediumDashed",
          "mediumDashDot",
          "mediumDashDotDot",
          "slantDashDot",
          "dashed",
          "dashDot",
          "dashDotDot",
          "hair",
        ].includes(border.style) ||
        typeof border.color !== "string" ||
        !/^#[0-9a-f]{6}$/i.test(border.color)
      )
        fail("Invalid cell border");
      return Object.freeze({ ...border });
    });
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      ...(multiple?{ranges}:{range:ranges[0]}),
      changes: Object.freeze(checkedChanges),
      borders: Object.freeze(checkedBorders),
    });
  }
  if (type === "paint-format") {
    if (
      !exact(value, [
        "type",
        "sourceSheetId",
        "sourceRange",
        "sheetId",
        "range",
      ]) ||
      !sheet(value.sourceSheetId) ||
      !sheet(value.sheetId)
    )
      fail("Invalid Format Painter command");
    const sourceRange = checkedSheetRange(value.sourceRange),
      range = checkedSheetRange(value.range);
    if (cells(sourceRange) > 10000 || cells(range) > 10000)
      fail("Format Painter selection exceeds 10000 cells");
    return Object.freeze({
      type,
      sourceSheetId: value.sourceSheetId,
      sourceRange,
      sheetId: value.sheetId,
      range,
    });
  }
  if (
    [
      "merge-cells",
      "unmerge-cells",
      "copy-range",
      'cut-range',
      "clear-range",
      "format-range",
    ].includes(type)
  ) {
    const formatting = type === "format-range";
    if (
      !exact(value, [
        "type",
        "sheetId",
        "range",
        ...(formatting ? ["path", "value"] : []),
      ]) ||
      !sheet(value.sheetId)
    )
      fail("Invalid range command");
    if (
      formatting &&
      (typeof value.value !== "string" ||
        !value.value.length ||
        value.value.length > 128 ||
        /[\ud800-\udfff]/u.test(value.value) ||
        ![
          "num_fmt",
          "alignment.horizontal",
          "font.b",
          "font.i",
          "font.u",
          "font.size",
        ].includes(value.path) ||
        (["font.b", "font.i", "font.u"].includes(value.path) &&
          !["true", "false"].includes(value.value)) ||
        (value.path === "font.size" && !fontPoints(value.value)) ||
        (value.path === "alignment.horizontal" &&
          !["general", "left", "center", "right"].includes(value.value)))
    )
      fail("Invalid range format");
    const range = checkedSheetRange(value.range);
    if (cells(range) > (type === "copy-range" ? 4096 : 10000))
      fail("Range exceeds the engine operation limit");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      range,
      ...(formatting ? { path: value.path, value: value.value } : {}),
    });
  }
  if (type === "paste-range") {
    const clipboard=clipboardFields(value);
    if (
      !exact(value, ["type", "sheetId", "row", "column", "mode",...Object.keys(clipboard)]) ||
      !sheet(value.sheetId) ||
      !coordinate(value.row, 1048576) ||
      !coordinate(value.column, 16384) ||
      !["all", "values", "formulas"].includes(value.mode)
    )
      fail("Invalid range paste");
    return Object.freeze({
      type,
      sheetId: value.sheetId,
      row: value.row,
      column: value.column,
      mode: value.mode,
      ...clipboard,
    });
  }
  const warning =
    type === "edit-cell" && Object.hasOwn(value, "allowValidationWarning");
  const fields = [
    "type",
    "sheetId",
    "row",
    "column",
    ...(type === "edit-cell" ? ["input"] : []),
    ...(warning ? ["allowValidationWarning"] : []),
  ];
  if (
    !exact(value, fields) ||
    !sheet(value.sheetId) ||
    !coordinate(value.row, 1048576) ||
    !coordinate(value.column, 16384)
  ) {
    fail("Cell coordinates must be zero-based logical worksheet indices");
  }
  if (
    type === "edit-cell" &&
    (typeof value.input !== "string" ||
      /[\ud800-\udfff]/u.test(value.input) ||
      new TextEncoder().encode(value.input).byteLength > SHEET_INPUT_MAX_BYTES)
  )
    fail("Cell input exceeds native text limits or contains invalid Unicode");
  if (warning && typeof value.allowValidationWarning !== "boolean")
    fail("Invalid validation warning choice");
  return Object.freeze({
    type,
    sheetId: value.sheetId,
    row: value.row,
    column: value.column,
    ...(type === "edit-cell" ? { input: value.input } : {}),
    ...(warning
      ? { allowValidationWarning: value.allowValidationWarning }
      : {}),
  });
}

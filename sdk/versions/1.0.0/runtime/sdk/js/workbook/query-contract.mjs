import { checkedDataToolOptions } from "./data-tools-contract.mjs";
import { CHART_KINDS } from "./chart-types.mjs";
import { checkedViewport } from "./viewport-contract.mjs";
import {checkedSheetFindOptions,checkedSheetFindTarget,checkedSheetReplaceRequest,checkedSheetFindResultSort} from './find-contract.mjs';
import {checkedSheetSpecialCellsRequest} from './special-cells-contract.mjs';
import {checkedSheetCommand} from './command-contract.mjs';
/** Transport bounds only. Query planning, comparison and indexes belong to Wasm. */
const encoder = new TextEncoder();
const integer = (value) => Number.isSafeInteger(value) && value >= 0;
const fail = (message) => {
  throw Object.assign(new TypeError(message), { code: "INVALID_QUERY" });
};
const schemas = {
  tables:['sheetId','offset','limit','literal'],
  'validation-table-rename':['table','name','columns'],
  'table-columns':['table','offset','limit'],
  'special-cells':['sheetId','ranges','kind','types'],
  'preview-replace':['options','replacement','target'],
  'find-matches':['options','after','limit'],
  'find-results':['options','sort','after','limit'],
  'navigation-destinations':['sheetId','offset','limit','literal'],
  'clipboard-text':['generation','offset','limit'],
  'data-validations':['sheetId','range','offset','limit'],
  'conditional-formats':['sheetId','range','offset','limit'],
  "validation-cell": ["sheetId", "row", "column", "input"],
  'validation-fill':['sheetId','ranges','origin','input'],
  'preview-selection':['sheetId','range','anchor','unit','operation'],
  "validation-list": [
    "sheetId",
    "row",
    "column",
    "ruleIndex",
    "offset",
    "limit",
    "literal",
  ],
  "defined-names": ["offset", "limit", "literal"],
  "used-range": ["sheetId"],
  "preview-remove-duplicates": [
    "sheetId",
    "range",
    "header",
    "columns",
    "caseSensitive",
  ],
  "preview-text-to-columns": [
    "sheetId",
    "range",
    "delimiter",
    "textQualifier",
    "collapseDelimiters",
    "destination",
    "mode",
  ],
  "charts-list": ["sheetId", "offset", "limit"],
  "chart-get": ["id"],
  "chart-change-type": ["id", "kind"],
  "chart-types": [],
  "chart-projection": ["id", "offset"],
  "chart-draft": ["sheetId", "range", "kind", "seriesByRows", "headers"],
  "cell-styles": [],
  "begin-filter": ["sheetId", "firstRow", "lastRow", "criteria"],
  "begin-distinct": [
    "sheetId",
    "firstRow",
    "lastRow",
    "column",
    "criteria",
    "ignoreOwnColumn",
  ],
  "begin-search": ["sheetId", "firstRow", "lastRow", "criteria", "search"],
  step: ["handle", "dataRevision", "budget"],
  "filter-page": ["handle", "dataRevision", "offset", "limit"],
  "distinct-page": [
    "handle",
    "dataRevision",
    "offset",
    "limit",
    "literal",
    "caseSensitive",
  ],
  "search-page": ["handle", "dataRevision"],
  "activate-filter": ["handle", "dataRevision"],
  "clear-filter": ["sheetId"],
  "filter-state": ["sheetId"],
  cancel: ["handle", "dataRevision"],
};

/** Reflected from the actual validator, rather than a separate automation API. */
export const SHEET_QUERY_ACTIONS = Object.freeze(Object.keys(schemas));
export function sheetQueryEffect(action) {
  if (!Object.hasOwn(schemas, action)) fail("Unsupported Sheet query");
  return ["activate-filter", "clear-filter"].includes(action) ? "mutation" : "query";
}
export function sheetQueryAvailable(action, capabilities) {
  if(!Object.hasOwn(schemas,action))return false;
  if(action==='validation-table-rename')return capabilities.includes('rename-table')&&capabilities.includes('data-validation');
  if(action==='tables'||action==='table-columns')return capabilities.includes('table-metadata');
  if(action==='validation-fill')return capabilities.includes('fill-selection');
  if(action==='preview-selection')return capabilities.includes('modify-selection');
  if(action==='special-cells')return capabilities.includes('special-cells');
  if(action==='clipboard-text')return capabilities.includes('paste-text');
  if(action==='find-results')return capabilities.includes('find-results')&&capabilities.includes('find-next');
  if(action==='find-matches')return capabilities.includes('find-next');
  if(action==='preview-replace')return capabilities.includes('replace-text');
  if(action==='navigation-destinations')return capabilities.includes('navigate-reference');
  if(action==='data-validations')return capabilities.includes('set-data-validation');
  return action!=='conditional-formats'||capabilities.includes('add-conditional-format');
}
function plain(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    Object.getPrototypeOf(value) === Object.prototype
  );
}
export function boundedQueryData(value) {
  let entries = 0;
  function copy(value, depth) {
    if (depth > 16 || ++entries > 4000)
      fail("Query exceeds bounded structure limits");
    if (value === null || typeof value === "boolean") return value;
    if (typeof value === "number") {
      if (!Number.isFinite(value)) fail("Query numbers must be finite");
      return value;
    }
    if (typeof value === "string") {
      if (value.length > 65536 || /[\ud800-\udfff]/u.test(value))
        fail("Query text must be bounded Unicode");
      return value;
    }
    if (Array.isArray(value)) {
      if (value.length > 4000)
        fail("Query array exceeds bounded structure limits");
      return Array.from(value, (item) => copy(item, depth + 1));
    }
    if (!plain(value)) fail("Query payload must contain plain JSON values");
    const keys = Object.keys(value);
    if (keys.length > 4000)
      fail("Query object exceeds bounded structure limits");
    return Object.fromEntries(
      keys.map((key) => {
        if (key.length > 128 || /[\ud800-\udfff]/u.test(key))
          fail("Invalid query field name");
        return [key, copy(value[key], depth + 1)];
      }),
    );
  }
  const result = copy(value, 0);
  if (encoder.encode(JSON.stringify(result)).byteLength > 65536)
    fail("Query payload exceeds 64 KiB");
  return result;
}
export function checkedQueryRequest(value) {
  const query = boundedQueryData(value);
  const fields =
    plain(query) &&
    typeof query.action === "string" &&
    Object.hasOwn(schemas, query.action) &&
    schemas[query.action];
  const optional =
    {
      "chart-draft": ["viewport"],
      "charts-list": ["viewport"],
      "chart-projection": ["token"],
      "cell-styles": ["offset", "limit", "literal"],
      "search-page": ["limit"],
      "begin-distinct": ["displayValues"],
      "begin-filter": ["firstColumn", "lastColumn"],
    }[query?.action] || [];
  const searchLimit =
    query?.action === "search-page" && Object.hasOwn(query, "limit");
  if (
    !fields ||
    Object.keys(query).some(
      (field) =>
        field !== "action" &&
        !fields.includes(field) &&
        !optional.includes(field),
    ) ||
    fields.some((field) => !Object.hasOwn(query, field))
  )
    fail("Invalid query action or fields");
  if(query.action==='validation-table-rename'){
    const {action,...data}=query;
    try{const {type,...checked}=checkedSheetCommand({type:'rename-table',...data});return {action,...checked};}catch(error){fail(error.message||'Invalid table heading validation');}
  }
  if(query.action==='tables'||query.action==='table-columns'){
    if(!integer(query.offset)||query.offset>(query.action==='tables'?256:16384)||!integer(query.limit)||query.limit<1||query.limit>64)fail('Invalid table metadata page');
    if(query.action==='tables'){
      if(query.sheetId!==null&&(typeof query.sheetId!=='string'||!query.sheetId.length||query.sheetId.length>128)||typeof query.literal!=='string'||encoder.encode(query.literal).length>512||/[\p{Cc}]/u.test(query.literal))fail('Invalid table search');
    }else if(typeof query.table!=='string'||!query.table.length||[...query.table].length>255||/[\p{Cc}]/u.test(query.table))fail('Invalid table identity');
    return query;
  }
  if(query.action==='preview-selection'){
    const {action,...data}=query;
    try{const {type,...checked}=checkedSheetCommand({type:'modify-selection',...data});return {action,...checked};}catch(error){fail(error.message||'Invalid selection preview');}
  }
  if(query.action==='validation-fill'){
    const {action,...data}=query;
    try{const {type,...checked}=checkedSheetCommand({type:'fill-selection',...data});return {action,...checked};}
    catch(error){fail(error.message||'Invalid fill validation');}
  }
  if(query.action==='special-cells'){
    const {action,...fields}=query;checkedSheetSpecialCellsRequest(fields);
  }
  if(query.action==='navigation-destinations'){
    if(typeof query.sheetId!=='string'||!query.sheetId.length||query.sheetId.length>128||!integer(query.offset)||query.offset>5120||!integer(query.limit)||query.limit<1||query.limit>32||typeof query.literal!=='string'||encoder.encode(query.literal).length>512||/[\p{Cc}]/u.test(query.literal))fail('Invalid navigation destination page');
  }
  if(query.action==='find-results'){
    if(!integer(query.limit)||query.limit<1||query.limit>16)fail('Invalid Find result page limit');
    return {action:query.action,options:checkedSheetFindOptions(query.options),sort:checkedSheetFindResultSort(query.sort),after:checkedSheetFindTarget(query.after),limit:query.limit};
  }
  if(query.action==='find-matches'){
    if(!integer(query.limit)||query.limit<1||query.limit>32)fail('Invalid Find page limit');
    return {action:query.action,options:checkedSheetFindOptions(query.options),after:checkedSheetFindTarget(query.after),limit:query.limit};
  }
  if(query.action==='preview-replace'){
    const {action,...request}=query;return {action,...checkedSheetReplaceRequest(request)};
  }
  if(query.action==='conditional-formats'||query.action==='data-validations'){
    if(typeof query.sheetId!=='string'||!query.sheetId.length||query.sheetId.length>128||!integer(query.offset)||query.offset>(query.action==='data-validations'?1024:4096)||!Number.isSafeInteger(query.limit)||query.limit<1||query.limit>32)fail('Invalid workbook rule page');
    if(query.range!==null){
      const r=query.range,keys=['firstRow','lastRow','firstColumn','lastColumn'];
      if(!plain(r)||Object.keys(r).length!==4||keys.some(key=>!Object.hasOwn(r,key)||!integer(r[key]))
        ||r.firstRow>r.lastRow||r.lastRow>=1048576||r.firstColumn>r.lastColumn||r.lastColumn>=16384)fail('Invalid workbook rule selection');
    }
  }
  if(query.action==='clipboard-text'&&(!integer(query.generation)||query.generation<1||!integer(query.offset)||query.offset>1024*1024
    ||!integer(query.limit)||query.limit<4||query.limit>8192))fail('Invalid clipboard text page');
  if (
    query.action === "validation-cell" &&
    (!integer(query.row) ||
      query.row >= 1048576 ||
      typeof query.input !== "string" ||
      encoder.encode(query.input).length > 16384 ||
      encoder.encode(JSON.stringify(query.input)).length > 24576)
  )
    fail("Invalid cell validation input");
  if (
    query.action === "validation-list" &&
    (!integer(query.row) ||
      query.row >= 1048576 ||
      !integer(query.ruleIndex) ||
      query.ruleIndex >= 1024 ||
      !integer(query.offset) ||
      query.offset > 10000 ||
      !integer(query.limit) ||
      query.limit < 1 ||
      query.limit > 32 ||
      typeof query.literal !== "string" ||
      Array.from(query.literal).length > 128)
  )
    fail("Invalid validation dropdown request");
  if (
    ["preview-remove-duplicates", "preview-text-to-columns"].includes(
      query.action,
    )
  ) {
    const { action, ...options } = query;
    return {
      action,
      ...checkedDataToolOptions(options, action.slice(8), {
        preview: true,
        fail,
      }),
    };
  }
  if (
    query.action === "defined-names" &&
    (!integer(query.offset) ||
      query.offset > 0xffffffff ||
      !integer(query.limit) ||
      query.limit < 1 ||
      query.limit > 100 ||
      typeof query.literal !== "string" ||
      encoder.encode(query.literal).length > 512 ||
      /[\p{Cc}]/u.test(query.literal))
  )
    fail("Invalid defined-name search or page");
  if (query.action === "charts-list") {
    if (
      !integer(query.offset) ||
      query.offset > 128 ||
      !integer(query.limit) ||
      query.limit < 1 ||
      query.limit > 16
    )
      fail("Invalid chart list page");
    if (query.viewport !== undefined)
      query.viewport = checkedViewport(query.viewport);
  }
  if (
    fields.includes("id") &&
    (!integer(query.id) || query.id < 1 || query.id > 0xffffffff)
  )
    fail("Invalid chart identity");
  if (query.action === "chart-change-type" && !CHART_KINDS.includes(query.kind))
    fail("Invalid chart type");
  if (
    query.action === "chart-projection" &&
    (!integer(query.offset) ||
      query.offset > 262144 ||
      (query.token !== undefined && (!integer(query.token) || query.token < 1)))
  )
    fail("Invalid chart projection cursor");
  if (query.action === "chart-draft") {
    if (query.viewport !== undefined)
      query.viewport = checkedViewport(query.viewport);
    const r = query.range,
      keys = ["firstRow", "lastRow", "firstColumn", "lastColumn"];
    if (
      !plain(r) ||
      Object.keys(r).length !== keys.length ||
      !keys.every((k) => integer(r[k])) ||
      r.firstRow > r.lastRow ||
      r.firstColumn > r.lastColumn ||
      r.lastRow >= 1048576 ||
      r.lastColumn >= 16384 ||
      !CHART_KINDS.includes(query.kind) ||
      typeof query.seriesByRows !== "boolean" ||
      typeof query.headers !== "boolean"
    )
      fail("Invalid chart data range");
  }
  if (
    query.action === "cell-styles" &&
    ((query.offset !== undefined &&
      (!integer(query.offset) || query.offset > 65535)) ||
      (query.limit !== undefined &&
        (!integer(query.limit) || query.limit < 1 || query.limit > 32)) ||
      (query.literal !== undefined &&
        (typeof query.literal !== "string" ||
          Array.from(query.literal).length > 128 ||
          encoder.encode(query.literal).length > 512)))
  )
    fail("Invalid cell style page");
  if (
    Object.hasOwn(query, "displayValues") &&
    typeof query.displayValues !== "boolean"
  )
    fail("Invalid display value mode");
  if (
    query.action === "begin-filter" &&
    (Object.hasOwn(query, "firstColumn") ||
      Object.hasOwn(query, "lastColumn")) &&
    (!integer(query.firstColumn) ||
      !integer(query.lastColumn) ||
      query.firstColumn > query.lastColumn ||
      query.lastColumn >= 16384)
  )
    fail("Invalid filter column domain");
  if (
    searchLimit &&
    (!integer(query.limit) || query.limit < 1 || query.limit > 64)
  )
    fail("Search page limit must be 1–64");
  if (
    fields.includes("sheetId") &&
    (typeof query.sheetId !== "string" ||
      !query.sheetId.length ||
      query.sheetId.length > 128)
  )
    fail("Invalid query sheet");
  if (
    fields.includes("firstRow") &&
    (!integer(query.firstRow) ||
      !integer(query.lastRow) ||
      query.firstRow > query.lastRow ||
      query.lastRow >= 1048576)
  )
    fail("Invalid query row domain");
  if (
    fields.includes("column") &&
    (!integer(query.column) || query.column >= 16384)
  )
    fail("Invalid query column");
  if (
    fields.includes("criteria") &&
    (!Array.isArray(query.criteria) || query.criteria.length > 32)
  )
    fail("Invalid query criteria");
  if (fields.includes("search") && !plain(query.search))
    fail("Invalid search options");
  if (
    fields.includes("handle") &&
    (!integer(query.handle) ||
      !query.handle ||
      query.handle > 0xffffffff ||
      !integer(query.dataRevision))
  )
    fail("Invalid query job cursor");
  if (
    fields.includes("budget") &&
    (!integer(query.budget) || query.budget < 64 || query.budget > 4096)
  )
    fail("Query step budget must be 64–4096");
  if (
    fields.includes("offset") &&
    !["charts-list", "chart-projection", "clipboard-text"].includes(query.action) &&
    (!integer(query.offset) ||
      query.offset > 0xffffffff ||
      !integer(query.limit) ||
      query.limit < 1 ||
      query.limit > (query.action === "distinct-page" ? 128 : 256))
  )
    fail("Invalid query page bounds");
  for (const field of ["ignoreOwnColumn", "caseSensitive"])
    if (fields.includes(field) && typeof query[field] !== "boolean")
      fail(`Invalid query ${field}`);
  if (
    fields.includes("literal") &&
    (typeof query.literal !== "string" ||
      encoder.encode(query.literal).length > 1024)
  )
    fail("Invalid query literal");
  return query;
}
export function checkedQueryResult(value) {
  const result = boundedQueryData(value);
  if (
    !plain(result) ||
    Object.keys(result).length !== 3 ||
    !integer(result.revision) ||
    !integer(result.dataRevision) ||
    (result.result !== null && !plain(result.result))
  )
    fail("Invalid query response");
  if (
    result.result?.revision !== undefined &&
    result.result.revision !== result.dataRevision
  )
    fail("Query page belongs to another data revision");
  return result;
}

const boundedText = (value, max) =>
  typeof value === "string" &&
  value.length <= max &&
  !/[\u0000\ud800-\udfff]/u.test(value);
export function checkedSelectionValidation(value) {
  if (value === undefined) return undefined;
  if (
    !value ||
    !Object.hasOwn(value, "prompt") ||
    !Object.hasOwn(value, "listRule") ||
    typeof value.checksInput !== "boolean" ||
    !(
      value.listRule === null ||
      (Number.isInteger(value.listRule) &&
        value.listRule >= 0 &&
        value.listRule <= 1023)
    ) ||
    (value.prompt !== null &&
      (!boundedText(value.prompt?.title, 128) ||
        !boundedText(value.prompt?.message, 1024)))
  )
    throw Error("Invalid selected cell validation.");
  return Object.freeze({
    listRule: value.listRule,
    checksInput: value.checksInput,
    prompt: value.prompt === null ? null : Object.freeze({ ...value.prompt }),
  });
}
/** Accept only feedback for the exact candidate cell and workbook revision. */
export function checkedCellValidation(response, cell, revision) {
  if (
    !Number.isSafeInteger(revision) ||
    revision < 0 ||
    response?.revision !== revision
  )
    throw Error("Workbook changed. Try entering the value again.");
  const value = response.result;
  if (
    !value ||
    value.sheetId !== cell.sheetId ||
    value.row !== cell.row ||
    value.column !== cell.column ||
    !Array.isArray(value.alerts) ||
    value.alerts.length > 32
  )
    throw Error("Cell validation could not be checked.");
  const alerts = value.alerts.map((alert) => {
    if (
      !alert ||
      !["stop", "warning", "information", "unsupported"].includes(
        alert.style,
      ) ||
      !boundedText(alert.title, 1024) ||
      !boundedText(alert.message, 8192)
    )
      throw Error("Cell validation could not be checked.");
    return Object.freeze({
      style: alert.style,
      title: alert.title,
      message: alert.message,
    });
  });
  const prompt = value.prompt;
  if (
    prompt != null &&
    (!boundedText(prompt.title, 1024) || !boundedText(prompt.message, 8192))
  )
    throw Error("Cell validation could not be checked.");
  return Object.freeze({
    alerts: Object.freeze(alerts),
    prompt: prompt == null ? null : Object.freeze({ ...prompt }),
  });
}

/** Bind table-heading feedback to its native identity and workbook revision.
 * The native overlay sees all proposed headings simultaneously. Only bounded
 * alert labels/coordinates cross this boundary, never candidate cell arrays. */
export function checkedTableRenameValidation(response, table, revision) {
  if (!Number.isSafeInteger(revision) || revision < 0 || response?.revision !== revision)
    throw Error('Workbook changed. Reopen Table Properties.');
  const value=response.result;
  if (!value || value.table!==table || Object.keys(value).some(key=>!['table','alerts'].includes(key))
    || !Array.isArray(value.alerts) || value.alerts.length>16)
    throw Error('Table headings could not be checked.');
  const alerts=value.alerts.map(alert=>{
    if (!alert || !['stop','warning','information','unsupported'].includes(alert.style)
      || !boundedText(alert.title,1024) || !boundedText(alert.message,8192)
      || !boundedText(alert.sheetId,128) || !alert.sheetId.length
      || !Number.isSafeInteger(alert.row) || alert.row<0 || alert.row>=1048576
      || !Number.isSafeInteger(alert.column) || alert.column<0 || alert.column>=16384
      || Object.keys(alert).some(key=>!['style','title','message','sheetId','row','column'].includes(key)))
      throw Error('Table headings could not be checked.');
    return Object.freeze({...alert});
  });
  return Object.freeze({table,alerts:Object.freeze(alerts)});
}

export function checkedValidationList(response, cell, revision, offset) {
  const value = response?.result;
  if (response?.revision !== revision)
    throw Error("Workbook changed. Reopen the list.");
  if (
    !value ||
    value.sheetId !== cell.sheetId ||
    value.row !== cell.row ||
    value.column !== cell.column ||
    value.ruleIndex !== cell.ruleIndex ||
    !Number.isInteger(value.total) ||
    value.total < 0 ||
    value.total > 10000 ||
    !Array.isArray(value.items) ||
    value.items.length > 32 ||
    offset + value.items.length > value.total ||
    (!value.items.length && offset < value.total) ||
    value.nextOffset !==
      (offset + value.items.length < value.total
        ? offset + value.items.length
        : null)
  )
    throw Error("Invalid validation list.");
  const seen = new Set();
  for (const item of value.items) {
    if (
      !item ||
      !Number.isInteger(item.index) ||
      item.index < 0 ||
      item.index >= 10000 ||
      seen.has(item.index) ||
      !boundedText(item.label, 256)
    )
      throw Error("Invalid validation list value.");
    seen.add(item.index);
  }
  return value;
}

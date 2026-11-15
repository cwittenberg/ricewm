import {
  Column,
  Container,
  Row,
  Label,
  TextField,
  CheckBox,
  Button,
} from "./controls.mjs";
import { Message } from "./messages.mjs";
import { TaskController } from "./tasks.mjs";
import { Control, State } from './control.mjs';

const clone = (value) => structuredClone(value);
const fieldId = (value) => {
  if (
    typeof value !== "string" ||
    !/^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(value)
  )
    throw TypeError("Invalid form field id");
  return value;
};
const selectedFields = (model, fields) => {
  if (fields === undefined) return [...model.fields.keys()];
  if (!Array.isArray(fields) || !fields.length || fields.length > model.fields.size ||
      new Set(fields).size !== fields.length || fields.some(id => !model.fields.has(id)))
    throw TypeError('Select known, distinct form fields');
  return [...fields];
};
/** One authoritative form state, independent from its native controls. */
export class FormModel {
  static controlFactory = Object.freeze({
    topics: ["forms", "workflow", "product"],
    constructor:
      'new FormModel({fields:[{id,label,initial,required?,validate?:async(value,values)=>null|string,parse?}],validate?:async(values)=>({fieldId:"error"}),values?})',
    methods:
      "set(id,value), get(id), validate({touch:true,fields?:[ids]}), submit(async(values)=>result,{fields?:[ids]}), reset(values?), subscribe(listener), destroy(); snapshot returns values, errors, touched, dirty, valid, busy, revision. Optional fields validate and accept only those fields, retaining other drafts, errors and dirty baselines; cross-form validation still runs. Work receives the full value snapshot. Validation is stale-result safe.",
  });
  constructor({ fields = [], validate = null, values = {} } = {}) {
    if (
      !Array.isArray(fields) ||
      fields.length > 256 ||
      (validate !== null && typeof validate !== "function")
    )
      throw TypeError("Invalid form");
    this.fields = new Map();
    for (const field of fields) {
      const id = fieldId(field.id);
      if (
        this.fields.has(id) ||
        (field.validate != null && typeof field.validate !== "function") ||
        (field.parse != null && typeof field.parse !== "function")
      )
        throw TypeError("Invalid form field");
      this.fields.set(id, { ...field, id });
    }
    this.initial = Object.fromEntries(
      [...this.fields].map(([id, field]) => [
        id,
        clone(Object.hasOwn(values, id) ? values[id] : (field.initial ?? "")),
      ]),
    );
    this.values = clone(this.initial);
    this.errors = {};
    this.touched = new Set();
    this.listeners = new Set();
    this.revision = 0;
    this.validation = 0;
    this.crossValidate = validate;
    this.task = new TaskController();
    this.stopTask = this.task.subscribe(() => this.changed());
  }
  get snapshot() {
    return Object.freeze({
      values: clone(this.values),
      errors: { ...this.errors },
      touched: [...this.touched],
      dirty: JSON.stringify(this.values) !== JSON.stringify(this.initial),
      valid: !Object.keys(this.errors).length,
      busy: this.task.state.status === "running",
      revision: this.revision,
      lastChangedField: this.lastChangedField ?? null,
    });
  }
  get(id) {
    if (!this.fields.has(id)) throw Error("Unknown field");
    return clone(this.values[id]);
  }
  set(id, value) {
    const field = this.fields.get(id);
    if (!field) throw Error("Unknown field");
    this.values[id] = clone(field.parse ? field.parse(value) : value);
    this.revision++;
    this.validation++;
    this.lastChangedField = id;
    this.touched.add(id);
    delete this.errors[id];
    this.changed();
  }
  subscribe(listener) {
    if (typeof listener !== "function")
      throw TypeError("Expected form listener");
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  changed() {
    for (const listener of [...this.listeners]) listener(this.snapshot);
  }
  async validate({ touch = true, fields } = {}) {
    const selected = selectedFields(this, fields);
    const serial = ++this.validation,
      revision = this.revision,
      values = clone(this.values),
      errors = {};
    if (touch) for (const id of selected) this.touched.add(id);
    await Promise.all(
      selected.map(async id => {
        const field = this.fields.get(id);
        const value = values[id];
        let message =
          field.required &&
          (value == null ||
            value === "" ||
            (Array.isArray(value) && !value.length))
            ? "This field is required"
            : null;
        if (!message && field.validate)
          message = await field.validate(value, values);
        if (message != null && message !== false && message !== "") {
          if (typeof message !== "string" || message.length > 1024)
            throw TypeError("Validator must return an error string or null");
          errors[id] = message;
        }
      }),
    );
    if (this.crossValidate) {
      const cross = await this.crossValidate(values);
      if (cross) {
        if (typeof cross !== "object" || Array.isArray(cross))
          throw TypeError("Form validation needs field errors");
        for (const [id, message] of Object.entries(cross)) {
          if (
            !this.fields.has(id) ||
            typeof message !== "string" ||
            message.length > 1024
          )
            throw TypeError("Invalid field error");
          if (message) errors[id] = message;
        }
      }
    }
    if (
      this.destroyed ||
      serial !== this.validation ||
      revision !== this.revision
    )
      return { valid: false, stale: true };
    if (fields === undefined) this.errors = errors;
    else {
      const retained = { ...this.errors };
      for (const id of selected) delete retained[id];
      this.errors = { ...retained, ...errors };
    }
    this.changed();
    return { valid: !Object.keys(errors).length, errors: { ...errors } };
  }
  submit(work, { fields } = {}) {
    const selected = selectedFields(this, fields);
    return this.task.run(async ({ signal }) => {
      const validation = await this.validate({ fields: fields === undefined ? undefined : selected });
      if (!validation.valid)
        throw Object.assign(Error("Correct the form errors"), {
          code: "INVALID_FORM",
          ...validation,
        });
      const revision = this.revision,
        result = await work(clone(this.values), { signal, revision });
      signal.throwIfAborted();
      if (this.revision === revision) {
        if (fields === undefined) this.initial = clone(this.values);
        else for (const id of selected) this.initial[id] = clone(this.values[id]);
      }
      this.changed();
      return result;
    });
  }
  reset(values = this.initial) {
    const next = Object.fromEntries(
      [...this.fields].map(([id]) => [
        id,
        clone(Object.hasOwn(values, id) ? values[id] : this.initial[id]),
      ]),
    );
    this.initial = clone(next);
    this.values = next;
    this.errors = {};
    this.touched.clear();
    this.lastChangedField = null;
    this.revision++;
    this.validation++;
    this.changed();
  }
  destroy() {
    this.destroyed = true;
    this.validation++;
    this.stopTask();
    this.task.close();
    this.listeners.clear();
  }
}

export class FormField extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze([
    "model",
    "field",
    "control",
    "description",
    "layout",
    "labelWidth",
    "controlWidth",
  ]);
  static authoring = Object.freeze({
    constructor:
      'new FormField({model,field:"id",control:new TextField(),description?,layout:"stacked"|"inline",labelWidth:148,controlWidth:null})',
    methods:
      "focus() targets a composite's visible native input when available and clears other focus requests within its retained window root; native label/input/help/error stay bound to FormModel. Checkbox binds checked; other controls bind value. Destroy releases bindings.",
  });
  constructor({ model, field, control, description = "", layout = 'stacked', labelWidth = 148, controlWidth = null, ...properties } = {}) {
    if (
      !(model instanceof FormModel) ||
      !model.fields.has(field) ||
      !control?.getProperty
    )
      throw TypeError("FormField needs a known field and native input");
    if (!['stacked', 'inline'].includes(layout) || !Number.isInteger(labelWidth) || labelWidth < 32 || labelWidth > 1024 ||
        controlWidth !== null && (!Number.isInteger(controlWidth) || controlWidth < 40 || controlWidth > 1024)) throw TypeError('Invalid form field layout');
    const spec = model.fields.get(field),
      error = new Label({ text: "", visible: false, invalid: true }),
      label = new Label({ text: spec.label || field });
    const content = layout === 'inline' ? [new Row({ text: 'panel:fixed:flush:gap=8' }, [
      new Container({ text: `panel:fixed:flush:width=${labelWidth}`, state: State.FLAT }, [label]),
      controlWidth === null ? control : new Container({ text: `panel:fixed:flush:width=${controlWidth}`, state: State.FLAT }, [control]),
    ])] : [label, control];
    super({ text: "panel:fixed:flush:gap=4", ...properties }, [
      ...content,
      ...(description ? [new Label({ text: description })] : []),
      error,
    ]);
    this.input = control;
    this.error = error;
    const key = control instanceof CheckBox ? "checked" : "value";
    control.setProperty("accessibleName", spec.label || field);
    const refresh = () => {
      const value = model.get(field);
      if (typeof control.setFormValue === 'function') control.setFormValue(value, { preserveDraft: model.lastChangedField != null });
      else if (!Object.is(control.getProperty(key), value))
        control.setProperty(key, value, { notify: false });
      error.text = model.errors[field] || "";
      error.visible = Boolean(error.text);
      control.setProperty("invalid", error.visible);
    };
    control.setProperty("required", spec.required === true);
    if (description) control.setProperty("accessibleDescription", description);
    this.stop = model.subscribe(refresh);
    this.stopInput = control.on(Message.VALUE_CHANGED, (event) => {
      model.set(
        field,
        key === "checked"
          ? control.getProperty("checked")
          : (event.detail?.value ?? control.getProperty("value")),
      );
    });
    this.stopDraft = control.on(Message.CHANGE, event => {
      if (event.target === control && event.detail?.draft === true) model.set(field, event.detail.value);
    });
    this.stopActivate =
      key === "checked"
        ? control.on(Message.ACTIVATE, () =>
            model.set(field, control.getProperty("checked")),
          )
        : () => {};
    refresh();
  }
  focus() {
    const editor = this.input.input;
    const target = editor instanceof Control && this.input.contains(editor) && editor.visible && editor.enabled ? editor : this.input;
    let root = this;
    while (root.parent) root = root.parent;
    const pending = [root];
    while (pending.length) {
      const control = pending.pop();
      if (control !== target && control.getProperty('focused')) control.setProperty('focused', false);
      pending.push(...control.children);
    }
    target.setProperty("focused", true);
    // A retained FOCUSED bit may already name this input after the user has
    // clicked Apply. An explicit selection request reasserts native focus
    // without recreating the editor or resetting its draft.
    if (target instanceof TextField) target.setSelection(0, Array.from(target.value).length);
  }
  destroy() {
    this.stop?.();
    this.stopInput?.();
    this.stopDraft?.();
    this.stopActivate?.();
    return super.destroy();
  }
}
export class Form extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze(["model", "controls", "rows", "fieldOptions"]);
  static authoring = Object.freeze({
    constructor:
      'new Form({model:new FormModel(...),controls:{fieldId:nativeControl},rows?:[["first","second"],["name"]],fieldOptions?:{first:{layout:"inline",labelWidth:148,controlWidth:120}}})',
    methods:
      "validate(), focusFirstError(); model owns validation and values. Unspecified inputs use TextField or CheckBox for boolean initial values. Optional rows place every field exactly once in caller-owned groups of 1–8 columns. Field options configure existing inline/stacked FormField layout without replacing inputs or their model bindings.",
  });
  constructor({ model, controls = {}, rows = null, fieldOptions = {}, ...properties } = {}) {
    if (!(model instanceof FormModel)) throw TypeError("Form needs FormModel");
    if (!fieldOptions || ![Object.prototype,null].includes(Object.getPrototypeOf(fieldOptions)) || Object.entries(fieldOptions).some(([id,options])=>!model.fields.has(id)||!options||![Object.prototype,null].includes(Object.getPrototypeOf(options))||Object.keys(options).some(key=>!['layout','labelWidth','controlWidth'].includes(key)))) throw TypeError('Invalid form field options');
    if(rows!==null&&(!Array.isArray(rows)||rows.length>128||rows.some(row=>!Array.isArray(row)||!row.length||row.length>8||row.some(id=>typeof id!=='string'||!model.fields.has(id)))||rows.flat().length!==model.fields.size||new Set(rows.flat()).size!==model.fields.size))throw TypeError('Form rows must contain every field exactly once');
    const fields = [...model.fields].map(
      ([id, spec]) =>
        new FormField({
          ...(Object.hasOwn(fieldOptions,id)?fieldOptions[id]:{}),
          model,
          field: id,
          control:
            controls[id] ||
            (typeof model.get(id) === "boolean"
              ? new CheckBox()
              : new TextField()),
        }),
    );
    const byId=new Map([...model.fields.keys()].map((id,index)=>[id,fields[index]]));
    const children=rows===null?fields:rows.map(row=>{
      if(row.length===1)return byId.get(row[0]);
      return new Row({text:'panel:fixed:flush:gap=18'},row.map(id=>{const field=byId.get(id);field.setProperty('state',field.state|State.EXPANSIVE);return field;}));
    });
    super({ text: "panel:fixed:gap=12", ...properties }, children);
    this.model = model;
    this.fields = new Map(
      [...model.fields.keys()].map((id, index) => [id, fields[index]]),
    );
  }
  focusFirstError() {
    this.fields.get(Object.keys(this.model.errors)[0])?.focus();
  }
  async validate() {
    const result = await this.model.validate();
    if (!result.valid) this.focusFirstError();
    return result;
  }
}

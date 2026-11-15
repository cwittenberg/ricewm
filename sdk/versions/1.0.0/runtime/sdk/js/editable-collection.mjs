import { CommandHistory } from "./command-history.mjs";
import {
  validateCapabilitySchema,
  validateCapabilityValue,
} from "./app-capabilities.mjs";
import { Column, Row, Label, Button } from "./controls.mjs";
import { Table } from "./data-table.mjs";
import { FormModel, Form } from "./forms.mjs";
import { Message } from "./messages.mjs";

/** Schema-checked records and undo transactions, independent from any view. */
export class RecordCollection {
  static controlFactory = Object.freeze({
    topics: ["tables", "forms", "workflow"],
    constructor:
      'new RecordCollection({schema:{type:"object",properties:{id:{type:"string"},...},required:["id"],additionalProperties:false},key:"id",rows:[],maximumRows:10000})',
    methods:
      "insert(row), patch(id,changes), remove(ids), get(id), subscribe(listener); history exposes shared immutable rows and undo/redo. Validation happens before any mutation.",
  });
  constructor({ schema, key = "id", rows = [], maximumRows = 10000 } = {}) {
    validateCapabilitySchema(schema);
    if (
      schema.type !== "object" ||
      schema.properties[key]?.type !== "string" ||
      !schema.required?.includes(key) ||
      !Number.isInteger(maximumRows) ||
      maximumRows < 1 ||
      maximumRows > 100000
    )
      throw TypeError(
        "Collections need a required string primary key and bounded rows",
      );
    Object.assign(this, { schema: structuredClone(schema), key, maximumRows });
    this.history = new CommandHistory({
      value: { rows },
      validate: (value) => {
        if (
          !value ||
          !Array.isArray(value.rows) ||
          value.rows.length > maximumRows ||
          Object.keys(value).some((k) => k !== "rows")
        )
          throw TypeError("Invalid collection data");
        const ids = new Set();
        for (const row of value.rows) {
          validateCapabilityValue(schema, row);
          if (!row[key] || ids.has(row[key]))
            throw TypeError("Record ids must be unique and nonempty");
          ids.add(row[key]);
        }
        return value;
      },
    });
  }
  get rows() {
    return this.history.value.rows;
  }
  get(id) {
    const row = this.rows.find((row) => row[this.key] === id);
    if (!row) throw Error("Unknown record");
    return row;
  }
  insert(row) {
    validateCapabilityValue(this.schema, row);
    if (this.rows.some((r) => r[this.key] === row[this.key]))
      throw Error("Record already exists");
    return this.history.transact("Add record", (draft) => {
      draft.rows.push(structuredClone(row));
    });
  }
  patch(id, changes) {
    this.get(id);
    if (
      !changes ||
      typeof changes !== "object" ||
      Array.isArray(changes) ||
      Object.hasOwn(changes, this.key)
    )
      throw TypeError("A record primary key cannot be changed");
    return this.history.transact("Edit record", (draft) => {
      const row = draft.rows.find((row) => row[this.key] === id);
      Object.assign(row, structuredClone(changes));
    });
  }
  remove(ids) {
    if (!Array.isArray(ids) || !ids.length || new Set(ids).size !== ids.length)
      throw TypeError("Choose unique record ids");
    for (const id of ids) this.get(id);
    const selected = new Set(ids);
    return this.history.transact("Delete records", (draft) => {
      draft.rows = draft.rows.filter((row) => !selected.has(row[this.key]));
    });
  }
  subscribe(listener) {
    return this.history.subscribe(listener);
  }
}

/** Native virtual table plus a validated, keyboard-accessible record editor. */
export class EditableTable extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze([
    "model",
    "columns",
    "newRow",
    "confirmDelete",
    "onError",
  ]);
  static authoring = Object.freeze({
    constructor:
      "new EditableTable({model:RecordCollection,columns?:Table columns,newRow?:()=>record,confirmDelete?:async(record)=>boolean,onError?})",
    methods:
      "edit(id), save(), deleteSelection(); native virtual table retains sort/selection and edits through one schema-checked undo transaction. Detail fields support text, numbers and booleans; arbitrary record schemas remain usable through RecordCollection and custom views.",
  });
  constructor({
    model,
    columns,
    newRow = null,
    confirmDelete = async () => true,
    onError = () => {},
    ...properties
  } = {}) {
    if (
      !(model instanceof RecordCollection) ||
      (newRow !== null && typeof newRow !== "function") ||
      typeof confirmDelete !== "function" ||
      typeof onError !== "function"
    )
      throw TypeError("Invalid editable table");
    const specs = Object.entries(model.schema.properties).filter(
      ([id]) => id !== model.key,
    );
    if (
      specs.some(
        ([, schema]) =>
          !["string", "number", "integer", "boolean"].includes(schema.type),
      )
    )
      throw TypeError("Use custom editors for structured record fields");
    const fields = specs.map(([id, schema]) => ({
        id,
        label: schema.description || id,
        initial: schema.type === "boolean" ? false : "",
        validate: (value) => {
          try {
            validateCapabilityValue(
              schema,
              ["number", "integer"].includes(schema.type)
                ? value === ""
                  ? NaN
                  : Number(value)
                : value,
            );
            return null;
          } catch (error) {
            return error.message;
          }
        },
      })),
      formModel = new FormModel({ fields }),
      form = new Form({ model: formModel }),
      table = new Table({
        rowKey: model.key,
        data: model.rows,
        columns:
          columns ??
          Object.keys(model.schema.properties).map((key) => ({
            key,
            title: key,
          })),
        state: 1 | 1048576,
      }),
      status = new Label({ text: "Select a record to edit" }),
      save = new Button({ text: "Save record", enabled: false }),
      remove = new Button({ text: "Delete record", enabled: false }),
      add = new Button({ text: "Add record", visible: newRow !== null });
    super({ text: "panel:gap=8", ...properties }, [
      table,
      status,
      form,
      new Row({ text: "panel:actions" }, [add, save, remove]),
    ]);
    Object.assign(this, {
      model,
      formModel,
      form,
      table,
      status,
      saveButton: save,
      deleteButton: remove,
      confirmDelete,
      onError,
      specs,
    });
    this.selected = null;
    this.editRevision = null;
    const refresh = () => {
      table.setData(model.rows);
      if (
        this.selected &&
        !model.rows.some((row) => row[model.key] === this.selected)
      ) {
        this.selected = null;
        save.enabled = remove.enabled = false;
        status.text = "Select a record to edit";
      }
    };
    this.stop = model.subscribe(refresh);
    table.on(Message.SELECT, (event) => {
      const id = event.detail?.rowIds?.[0];
      if (id !== undefined) this.edit(id);
    });
    save.on(Message.ACTIVATE, () => this.save().catch(onError));
    remove.on(Message.ACTIVATE, () => this.deleteSelection().catch(onError));
    add.on(Message.ACTIVATE, () => {
      try {
        const row = newRow();
        model.insert(row);
        this.edit(row[model.key]);
      } catch (error) {
        onError(error);
      }
    });
    for (const field of form.fields.values()) field.input.enabled = false;
  }
  edit(id) {
    const row = this.model.get(id);
    this.selected = id;
    this.editRevision = this.model.history.revision;
    this.formModel.reset(
      Object.fromEntries(
        this.specs.map(([id, schema]) => [
          id,
          row[id] ?? (schema.type === "boolean" ? false : ""),
        ]),
      ),
    );
    for (const field of this.form.fields.values()) field.input.enabled = true;
    this.saveButton.enabled = this.deleteButton.enabled = true;
    this.status.text = "Editing " + id;
    return row;
  }
  save() {
    return this.formModel.submit(async (values) => {
      if (this.selected === null) throw Error("Choose a record");
      if (this.editRevision !== this.model.history.revision)
        throw Error(
          "Collection changed; select the record again before saving",
        );
      const id = this.selected,
        changes = Object.fromEntries(
          this.specs.map(([id, schema]) => [
            id,
            ["number", "integer"].includes(schema.type)
              ? Number(values[id])
              : values[id],
          ]),
        );
      this.model.patch(id, changes);
      this.edit(id);
      this.status.text = "Record saved";
      return true;
    });
  }
  async deleteSelection() {
    if (this.selected === null) return false;
    const id = this.selected,
      row = this.model.get(id),
      revision = this.model.history.revision;
    if ((await this.confirmDelete(row)) !== true) return false;
    if (this.model.history.revision !== revision || this.selected !== id)
      throw Error("Collection changed during confirmation");
    this.model.remove([id]);
    return true;
  }
  destroy() {
    this.stop?.();
    this.formModel.destroy();
    return super.destroy();
  }
}

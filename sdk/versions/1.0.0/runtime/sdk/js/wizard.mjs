import { Column, Row, Label, Button } from "./controls.mjs";
import { Message } from "./messages.mjs";
import { FormModel } from "./forms.mjs";
import { TaskController } from "./tasks.mjs";

/** Branching workflow state; native page controls are retained by Wizard. */
export class WizardController {
  static controlFactory = Object.freeze({
    topics: ["workflow", "forms", "product"],
    constructor:
      "new WizardController({steps:[{id,title,form?:FormModel,validate?:async(state)=>boolean,next?:id|((state)=>id|null)}],state:{},finish:async(state,{signal})=>result,confirmCancel?:async(state)=>boolean,checkpoint?:async(snapshot)=>{}})",
    methods:
      "next(), back(), finish(), cancel(), restore(snapshot), reconcileFinish({completed}), subscribe(listener); snapshot reports current/history/state/status/busy. Validates before transitions; failed transitions retain current page. Interrupted finishing/failed checkpoints require checking the external result before explicit reconcileFinish; finish is never automatically repeated.",
  });
  constructor({
    steps = [],
    state = {},
    finish = async () => null,
    confirmCancel = async () => true,
    checkpoint = null,
  } = {}) {
    if (
      !Array.isArray(steps) ||
      !steps.length ||
      steps.length > 64 ||
      typeof finish !== "function" ||
      typeof confirmCancel !== "function" ||
      (checkpoint !== null && typeof checkpoint !== "function")
    )
      throw TypeError("Invalid wizard");
    this.steps = new Map();
    for (const step of steps) {
      if (
        !step ||
        typeof step.id !== "string" ||
        !/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/.test(step.id) ||
        this.steps.has(step.id)
      )
        throw TypeError("Wizard steps need unique ids");
      if (step.form != null && !(step.form instanceof FormModel))
        throw TypeError("Invalid wizard form");
      this.steps.set(step.id, { ...step });
    }
    this.order = [...this.steps.keys()];
    this.current = this.order[0];
    this.history = [];
    this.data = structuredClone(state);
    this.status = "active";
    this.finishWork = finish;
    this.confirmCancel = confirmCancel;
    this.checkpoint = checkpoint;
    this.listeners = new Set();
    this.task = new TaskController();
    this.stopTask = this.task.subscribe(() => this.changed());
  }
  get snapshot() {
    return {
      version: 1,
      current: this.current,
      history: [...this.history],
      state: structuredClone(this.data),
      status: this.status,
      busy: this.task.state.status === "running",
      forms: Object.fromEntries(
        [...this.steps]
          .filter(([, step]) => step.form)
          .map(([id, step]) => [id, step.form.snapshot.values]),
      ),
    };
  }
  subscribe(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  changed() {
    for (const listener of [...this.listeners]) listener(this.snapshot);
  }
  async persist() {
    await this.checkpoint?.(this.snapshot);
  }
  target() {
    const step = this.steps.get(this.current);
    return typeof step.next === "function"
      ? step.next(this.snapshot)
      : step.next !== undefined
        ? step.next
        : (this.order[this.order.indexOf(this.current) + 1] ?? null);
  }
  async validate() {
    const step = this.steps.get(this.current);
    if (step.form && !(await step.form.validate()).valid) return false;
    return !step.validate || (await step.validate(this.snapshot)) === true;
  }
  transition(work) {
    if (this.status !== "active")
      return Promise.reject(Error("Wizard is no longer active"));
    return this.task.run(work);
  }
  next() {
    return this.transition(async ({ signal }) => {
      if (!(await this.validate())) return false;
      signal.throwIfAborted();
      const target = this.target();
      if (target === null) throw Error("Use finish on the last step");
      if (!this.steps.has(target) || target === this.current)
        throw Error("Invalid wizard branch");
      const previous = this.snapshot;
      this.history.push(this.current);
      this.current = target;
      try {
        await this.persist();
        signal.throwIfAborted();
      } catch (error) {
        this.current = previous.current;
        this.history = previous.history;
        throw error;
      }
      this.changed();
      return true;
    });
  }
  back() {
    return this.transition(async ({ signal }) => {
      if (!this.history.length) return false;
      const previous = this.snapshot;
      this.current = this.history.pop();
      try {
        await this.persist();
        signal.throwIfAborted();
      } catch (error) {
        this.current = previous.current;
        this.history = previous.history;
        throw error;
      }
      this.changed();
      return true;
    });
  }
  finish() {
    return this.transition(async ({ signal }) => {
      if (this.target() !== null) throw Error("Finish requires the final step");
      if (!(await this.validate())) return false;
      signal.throwIfAborted();
      this.status = "finishing";
      try {
        await this.persist();
        signal.throwIfAborted();
      } catch (error) {
        this.status = "active";
        throw error;
      }
      this.changed();
      try {
        const result = await this.finishWork(this.snapshot, { signal });
        signal.throwIfAborted();
        this.status = "completed";
        await this.persist();
        this.changed();
        return { completed: true, result };
      } catch (error) {
        if (this.status !== "completed") this.status = "failed";
        await this.persist().catch(() => {});
        this.changed();
        throw error;
      }
    });
  }
  reconcileFinish({ completed } = {}) {
    if (
      this.task.state.status === "running" ||
      !["failed", "finishing"].includes(this.status) ||
      typeof completed !== "boolean"
    )
      return Promise.reject(
        Error(
          "Reconcile only an inactive interrupted finish after checking its external result",
        ),
      );
    return this.task.run(async () => {
      const previous = this.status;
      this.status = completed ? "completed" : "active";
      try {
        await this.persist();
      } catch (error) {
        this.status = previous;
        throw error;
      }
      this.changed();
    });
  }
  async cancel() {
    if (this.status !== "active") return false;
    if (!(await this.confirmCancel(this.snapshot))) return false;
    this.task.cancel();
    this.status = "cancelled";
    await this.persist();
    this.changed();
    return true;
  }
  restore(value) {
    if (
      this.task.state.status === "running" ||
      !value ||
      value.version !== 1 ||
      !this.steps.has(value.current) ||
      !Array.isArray(value.history) ||
      value.history.length > 256 ||
      value.history.some((id) => !this.steps.has(id)) ||
      !["active", "cancelled", "completed", "finishing", "failed"].includes(
        value.status,
      )
    )
      throw TypeError("Invalid wizard checkpoint");
    const restored = structuredClone(value);
    for (const [id, values] of Object.entries(restored.forms || {})) {
      const form = this.steps.get(id)?.form;
      if (
        !form ||
        !values ||
        typeof values !== "object" ||
        Array.isArray(values) ||
        Object.keys(values).some((key) => !form.fields.has(key))
      )
        throw TypeError("Unknown checkpoint form or field");
    }
    for (const [id, values] of Object.entries(restored.forms || {}))
      this.steps.get(id).form.reset(values);
    this.current = restored.current;
    this.history = restored.history;
    this.data = restored.state;
    this.status = restored.status;
    this.changed();
  }
  close() {
    this.stopTask();
    this.task.close();
    this.listeners.clear();
  }
}
export class WizardStep extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze(["id", "title"]);
  static authoring = Object.freeze({
    constructor: "new WizardStep({id,title},[native controls])",
    behavior:
      "Pages belong to Wizard and stay mounted across Back/Next. The matching controller step owns validation/branching.",
  });
  constructor({ id, title, ...properties } = {}, children = []) {
    super(properties, children);
    this.stepId = id;
    this.stepTitle = title || id;
  }
}
export class Wizard extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze([
    "controller",
    "pages",
    "onError",
    "onComplete",
  ]);
  static authoring = Object.freeze({
    constructor:
      "new Wizard({controller,pages:[new WizardStep({id,title},[...])],onError?,onComplete?})",
    behavior:
      "Native heading, retained pages, Back/Next/Finish/Cancel and error feedback. Keyboard activation uses native buttons. Form errors focus their first native input when the page contains Form.",
  });
  constructor({
    controller,
    pages = [],
    onError = () => {},
    onComplete = () => {},
    ...properties
  } = {}) {
    if (
      !(controller instanceof WizardController) ||
      !Array.isArray(pages) ||
      pages.length !== controller.steps.size ||
      pages.some(
        (page) =>
          !(page instanceof WizardStep) || !controller.steps.has(page.stepId),
      ) ||
      new Set(pages.map((page) => page.stepId)).size !== pages.length
    )
      throw TypeError("Wizard needs exactly one page per step");
    const heading = new Label({ text: "" }),
      error = new Label({ text: "", visible: false, invalid: true }),
      back = new Button({ name: "wizard-back", text: "Back" }),
      next = new Button({ name: "wizard-next", text: "Next", accent: true }),
      finish = new Button({
        name: "wizard-finish",
        text: "Finish",
        accent: true,
      }),
      cancel = new Button({ name: "wizard-cancel", text: "Cancel" });
    super({ text: "panel:padding=16:gap=12", ...properties }, [
      heading,
      ...pages,
      error,
      new Row({ text: "panel:actions" }, [back, next, finish, cancel]),
    ]);
    this.controller = controller;
    const refresh = () => {
      const state = controller.snapshot;
      if (["finishing", "failed"].includes(state.status) && !state.busy) {
        error.text =
          "The final operation was interrupted. Check its result before resuming.";
        error.visible = true;
      }
      heading.text = controller.steps.get(state.current).title || state.current;
      for (const page of pages) page.visible = page.stepId === state.current;
      const active = state.status === "active";
      back.enabled = active && !state.busy && state.history.length > 0;
      next.visible = controller.target() !== null;
      finish.visible = !next.visible;
      next.enabled = finish.enabled = active && !state.busy;
      cancel.enabled = active;
    };
    const act = (work) => async () => {
      error.visible = false;
      try {
        const result = await work();
        if (result === false) {
          const page = pages.find((page) => page.stepId === controller.current);
          const focus = (control) => {
            if (typeof control.focusFirstError === "function")
              control.focusFirstError();
            for (const child of control.children) focus(child);
          };
          focus(page);
        }
        if (result?.completed) onComplete(result.result);
      } catch (cause) {
        error.text = String(cause.message || cause);
        error.visible = true;
        onError(cause);
      }
      refresh();
    };
    back.on(
      Message.ACTIVATE,
      act(() => controller.back()),
    );
    next.on(
      Message.ACTIVATE,
      act(() => controller.next()),
    );
    finish.on(
      Message.ACTIVATE,
      act(() => controller.finish()),
    );
    cancel.on(
      Message.ACTIVATE,
      act(() => controller.cancel()),
    );
    this.stop = controller.subscribe(refresh);
    refresh();
  }
  destroy() {
    this.stop?.();
    return super.destroy();
  }
}

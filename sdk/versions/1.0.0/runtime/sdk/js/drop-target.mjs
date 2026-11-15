import { desktopEventTopics } from "./desktop.mjs";
import { TaskController } from "./tasks.mjs";
/** Generic file/typed-data drop onto real retained controls. Copy semantics. */
export class DropTargetController {
  static controlFactory = Object.freeze({
    topics: ["visual", "documents", "workflow"],
    constructor:
      'new DropTargetController({window,control,bus,services,kind:"files"|"data",mimeTypes:[],onDrop:async({files,payload,signal})=>true,onHover?,onError?})',
    methods:
      "await ready, refresh(), close(); register after controls attach. Files arrive as read-only, worker-bound grants, never raw host paths. Typed data is bounded to 512 KiB. Return true only after validated atomic import; failed/cancelled drops reject without moving/deleting source files. Dynamic descendants need refresh(). Declare drops.register/unregister/respond and exact desktopEventTopics(appId).dropEvent subscription.",
  });
  constructor({
    window,
    control,
    bus,
    services,
    kind = "files",
    mimeTypes = [],
    onDrop,
    onHover = () => {},
    onError = () => {},
  } = {}) {
    if (
      !window?.application ||
      control?.application !== window.application ||
      window.application.controlWindows.get(control.id) !== window ||
      !bus ||
      !services?.drops ||
      typeof onDrop !== "function" ||
      typeof onHover !== "function" ||
      typeof onError !== "function"
    )
      throw TypeError(
        "Drop targets require an attached control in a live app-owned window",
      );
    Object.assign(this, {
      window,
      control,
      bus,
      services,
      kind,
      mimeTypes,
      onDrop,
      onHover,
      onError,
    });
    this.controlId = String(control.id);
    this.task = new TaskController();
    this.closed = false;
    this.stop = bus.subscribe(
      desktopEventTopics(window.application.manifest.id).dropEvent,
      (event) => {
        const data = event.payload;
        if (
          data?.window !== String(window.id) ||
          data.control !== this.controlId ||
          this.closed
        )
          return;
        if (data.phase !== "drop") {
          onHover(data);
          return;
        }
        return this.task
          .run(async ({ signal }) => {
            let accepted = false,
              error;
            try {
              accepted = (await onDrop({ ...data, signal })) === true;
              signal.throwIfAborted();
            } catch (cause) {
              error = String(cause.message || cause);
              onError(cause);
            } finally {
              onHover({ ...data, phase: "finished" });
            }
            await services.drops.respond({
              invocation: data.invocation,
              accepted,
              ...(error ? { error } : {}),
            });
          })
          .catch(async (cause) => {
            onError(cause);
            await services.drops
              .respond({
                invocation: data.invocation,
                accepted: false,
                error: String(cause.message || cause),
              })
              .catch(() => {});
          });
      },
    );
    this.stopDestroy = control.on("rice.destroy", () => {
      void this.close().catch(() => {});
    });
    this.ready = this.refresh();
    this.ready.catch(onError);
  }
  refresh() {
    if (this.closed || this.control.destroyed)
      return Promise.reject(Error("Drop target closed"));
    const controls = [],
      visit = (control) => {
        if (controls.length >= 256)
          throw RangeError("Drop regions contain at most 256 controls");
        controls.push(String(control.id));
        for (const child of control.children) visit(child);
      };
    visit(this.control);
    return this.services.drops.register({
      window: String(this.window.id),
      control: this.controlId,
      controls,
      kind: this.kind,
      mimeTypes: this.mimeTypes,
    });
  }
  async close() {
    if (this.closed) return;
    this.closed = true;
    this.stop();
    this.stopDestroy();
    this.task.close();
    await this.services.drops.unregister({
      window: String(this.window.id),
      control: this.controlId,
    });
  }
}

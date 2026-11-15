/** Browser accessibility projection of the active app's retained native tree.
 * No app markup is interpreted and hidden/other-workspace windows are excluded. */
export class RiceNativeAccessibility {
  constructor({ document, canvas, onAction, onError = () => {} } = {}) {
    Object.assign(this, { document, canvas, onAction, onError });
    this.elements = new Map();
    if (
      !document?.createElement ||
      !canvas?.parentNode ||
      typeof onAction !== "function"
    )
      return;
    this.root = document.createElement("section");
    this.root.setAttribute("aria-label", "Native application");
    this.root.style.cssText =
      "position:fixed;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap";
    canvas.parentNode.append(this.root);
  }
  update(snapshot) {
    if (!this.root) return;
    const tree = snapshot?.accessibilityTree;
    if (!tree || !Array.isArray(tree.nodes) || tree.nodes.length > 4096) {
      this.clear();
      return;
    }
    const app = String(tree.app);
    if (app !== this.app) {
      this.clear();
      this.app = app;
    }
    this.revision = String(tree.revision);
    this.root.setAttribute("aria-label", String(tree.title || "Application"));
    const seen = new Set();
    for (const node of tree.nodes) {
      if (
        !node ||
        typeof node.id !== "string" ||
        node.id.length > 32 ||
        seen.has(node.id) ||
        (node.parent !== null && !seen.has(node.parent))
      )
        continue;
      seen.add(node.id);
      const tag =
        node.role === "textbox"
          ? "textarea"
          : [
                "button",
                "checkbox",
                "radio",
                "tab",
                "menuitem",
                "treeitem",
                "listitem",
              ].includes(node.role)
            ? "button"
            : "div";
      let record = this.elements.get(node.id);
      if (record && record.tag !== tag) {
        record.element.remove();
        this.elements.delete(node.id);
        record = null;
      }
      if (!record) {
        const element = this.document.createElement(tag);
        record = { element, tag, node };
        if (tag !== "textarea") {
          record.label = this.document.createElement("span");
          element.append(record.label);
        }
        this.elements.set(node.id, record);
        if (tag === "button") {
          element.type = "button";
          element.addEventListener("click", (event) => {
            event.stopPropagation();
            this.act(record, "activate");
          });
        }
        if (tag === "textarea")
          element.addEventListener("change", () =>
            this.act(record, "set-field", element.value),
          );
      }
      record.node = node;
      const element = record.element,
        proposed = node.parent === null ? null : this.elements.get(node.parent),
        parent =
          !proposed || proposed.tag === "button" || proposed.tag === "textarea"
            ? this.root
            : proposed.element;
      if (element.parentNode !== parent) parent.append(element);
      if (node.role === "text") element.removeAttribute("role");
      else element.setAttribute("role", node.role);
      if (node.name) element.setAttribute("aria-label", String(node.name));
      else element.removeAttribute("aria-label");
      element.setAttribute("aria-disabled", String(node.disabled === true));
      element.setAttribute("aria-invalid", String(node.invalid === true));
      if (node.role === "dialog")
        element.setAttribute("aria-modal", String(node.modal === true));
      if (node.description)
        element.setAttribute("aria-description", node.description);
      else element.removeAttribute("aria-description");
      if (node.live) element.setAttribute("aria-live", node.live);
      else element.removeAttribute("aria-live");
      if (["checkbox", "radio"].includes(node.role))
        element.setAttribute("aria-checked", String(node.checked === true));
      if (["tab", "treeitem", "listitem"].includes(node.role))
        element.setAttribute("aria-selected", String(node.selected === true));
      if (tag === "textarea") {
        element.disabled = node.disabled === true;
        element.readOnly = node.readOnly === true || node.password === true;
        element.setAttribute("aria-required", String(node.required === true));
        if (node.password) {
          element.value = "";
          element.setAttribute(
            "aria-label",
            (node.name || "Password") + " · use the native password field",
          );
        } else if (this.document.activeElement !== element)
          element.value = typeof node.value === "string" ? node.value : "";
      } else {
        if (record.label.textContent !== String(node.name || ""))
          record.label.textContent = String(node.name || "");
        if (tag === "button") element.disabled = node.disabled === true;
      }
    }
    for (const [id, record] of this.elements)
      if (!seen.has(id)) {
        record.element.remove();
        this.elements.delete(id);
      }
  }
  act(record, operation, value) {
    if (
      !this.root ||
      record.node.disabled ||
      (operation === "set-field" &&
        (record.node.readOnly || record.node.password))
    )
      return;
    Promise.resolve(
      this.onAction({
        app: this.app,
        target: record.node.id,
        operation,
        ...(value === undefined ? {} : { value }),
      }),
    ).catch(this.onError);
  }
  clear() {
    for (const record of this.elements.values()) record.element.remove();
    this.elements.clear();
    this.app = null;
  }
  dispose() {
    this.clear();
    this.root?.remove();
    this.root = null;
  }
}

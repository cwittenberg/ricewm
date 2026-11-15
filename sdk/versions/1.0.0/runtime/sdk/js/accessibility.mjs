const roles = new Map([
  [10, "text"],
  [11, "img"],
  [12, "button"],
  [13, "button"],
  [14, "checkbox"],
  [15, "radio"],
  [16, "button"],
  [17, "button"],
  [20, "textbox"],
  [21, "textbox"],
  [24, "textbox"],
  [30, "menubar"],
  [31, "menu"],
  [32, "menuitem"],
  [33, "toolbar"],
  [34, "tablist"],
  [36, "slider"],
  [42, "tree"],
  [40, "list"],
  [41, "listitem"],
  [44, "treeitem"],
  [50, "dialog"],
]);
/** Semantic data follows retained visible controls; passwords never leave them. */
export function accessibilitySnapshot(window) {
  if (!window?.root || window.presentationState.closed)
    return { revision: "closed", nodes: [] };
  const nodes = [];
  const visit = (control, parent = null, disabled = false) => {
    if (
      !control.visible ||
      control.destroyed ||
      !window.rendered.has(control.id) ||
      nodes.length >= 4096
    )
      return;
    const values = control.values,
      kind = control.kind,
      role =
        values.accessibleRole ||
        (kind === 31 && control.parent?.kind === 30
          ? "menuitem"
          : roles.get(kind)) ||
        "group",
      password = kind === 24,
      raw = typeof values.value === "string" ? values.value : "",
      truncated = raw.length > 8192;
    nodes.push({
      id: String(control.id),
      parent,
      role,
      name: String(
        values.accessibleName ||
          ([
            "group",
            "table",
            "tree",
            "list",
            "tablist",
            "menubar",
            "toolbar",
          ].includes(role)
            ? ""
            : values.text) ||
          "",
      ).slice(0, 1024),
      displayText: kind === 10 ? String(values.text || "").slice(0, 1024) : "",
      modal: kind === 50,
      description: String(values.accessibleDescription || "").slice(0, 2048),
      disabled: disabled || !control.enabled,
      focused: values.focused === true,
      checked: values.checked === true,
      selected: values.selected === true,
      invalid: values.invalid === true,
      required: values.required === true,
      password,
      readOnly: values.readOnly === true || truncated,
      ...(role === "textbox" && !password
        ? { value: raw.slice(0, 8192), truncated }
        : {}),
      ...(values.accessibleLive ? { live: values.accessibleLive } : {}),
    });
    for (const child of control.children)
      visit(child, String(control.id), disabled || !control.enabled);
  };
  visit(window.root);
  return { revision: String(window.sequence), nodes };
}
accessibilitySnapshot.controlFactory = Object.freeze({
  topics: ["accessibility", "forms", "windows"],
  signature: "accessibilitySnapshot(window)",
  behavior:
    "Returns actual retained visible native semantics, roles, names/descriptions, focus, validation and relationships. Password values are excluded. Long text is explicitly truncated and read-only in the browser accessibility projection. This is not a claim of qualification for every assistive technology or input method.",
});

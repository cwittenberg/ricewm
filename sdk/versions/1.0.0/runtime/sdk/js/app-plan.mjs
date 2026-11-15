import { SDK_RESOURCE_PROFILES } from "./resource-profiles.mjs";
const id = (value) =>
  typeof value === "string" && /^[A-Za-z][A-Za-z0-9_.-]{0,95}$/.test(value);
const text = (value) =>
  typeof value === "string" && value.trim() && value.length <= 2048;
/** A reviewable dependency/workflow contract for complete generated apps. */
export function validateAppPlan(plan, { capabilities = [], files = {} } = {}) {
  if (
    !plan ||
    typeof plan !== "object" ||
    Array.isArray(plan) ||
    plan.schema !== "ricewm.app-plan/v1" ||
    Object.keys(plan).some(
      (key) =>
        ![
          "schema",
          "requirements",
          "windows",
          "state",
          "journeys",
          "resources",
        ].includes(key),
    ) ||
    !Array.isArray(plan.requirements) ||
    !plan.requirements.length ||
    plan.requirements.length > 64 ||
    plan.requirements.some((value) => !text(value)) ||
    !Array.isArray(plan.windows) ||
    !plan.windows.length ||
    plan.windows.length > 32 ||
    !Array.isArray(plan.state) ||
    !plan.state.length ||
    plan.state.length > 64 ||
    !Array.isArray(plan.journeys) ||
    !plan.journeys.length ||
    plan.journeys.length > 32 ||
    !Array.isArray(plan.resources) ||
    new Set(plan.resources).size !== plan.resources.length ||
    plan.resources.some(
      (resource) => !Object.hasOwn(SDK_RESOURCE_PROFILES.profiles, resource),
    )
  )
    throw TypeError(
      "Complete apps need a bounded app plan with requirements, windows, state, journeys and selected SDK resources",
    );
  const windows = new Map();
  for (const window of plan.windows) {
    if (
      !window ||
      Object.keys(window).some(
        (key) => !["id", "purpose", "owner"].includes(key),
      ) ||
      !id(window.id) ||
      windows.has(window.id) ||
      !text(window.purpose) ||
      (window.owner !== undefined && window.owner !== null && !id(window.owner))
    )
      throw TypeError("Plan windows need unique ids and purposes");
    windows.set(window.id, window);
  }
  for (const window of windows.values()) {
    const seen = new Set([window.id]);
    let owner = window.owner;
    while (owner) {
      if (seen.has(owner) || !windows.has(owner))
        throw TypeError("Window ownership must be an acyclic app-owned tree");
      seen.add(owner);
      owner = windows.get(owner).owner;
    }
  }
  const state = new Set();
  for (const entry of plan.state) {
    if (
      !entry ||
      Object.keys(entry).some(
        (key) => !["id", "scope", "module", "persistence"].includes(key),
      ) ||
      !id(entry.id) ||
      state.has(entry.id) ||
      !["document", "session", "window"].includes(entry.scope) ||
      typeof entry.module !== "string" ||
      !Object.hasOwn(files, entry.module) ||
      !entry.module.endsWith(".mjs") ||
      !["none", "settings", "files"].includes(entry.persistence)
    )
      throw TypeError(
        "Plan state needs one real source module, scope and explicit persistence choice",
      );
    state.add(entry.id);
  }
  const journeys = new Set(),
    covered = new Set();
  for (const journey of plan.journeys) {
    if (
      !journey ||
      Object.keys(journey).some(
        (key) =>
          !["id", "title", "windows", "steps", "capabilities"].includes(key),
      ) ||
      !id(journey.id) ||
      journeys.has(journey.id) ||
      !text(journey.title) ||
      !Array.isArray(journey.windows) ||
      !journey.windows.length ||
      journey.windows.some((id) => !windows.has(id)) ||
      !Array.isArray(journey.steps) ||
      !journey.steps.length ||
      journey.steps.length > 32 ||
      journey.steps.some((step) => !text(step)) ||
      !Array.isArray(journey.capabilities) ||
      journey.capabilities.some((id) => !capabilities.some((c) => c.id === id))
    )
      throw TypeError(
        "Plan journeys must reference declared windows, capabilities and concrete steps",
      );
    journeys.add(journey.id);
    for (const capability of journey.capabilities) covered.add(capability);
  }
  if (capabilities.some((capability) => !covered.has(capability.id)))
    throw TypeError(
      "Every declared capability needs a planned acceptance journey",
    );
  const source = JSON.stringify(plan);
  if (new TextEncoder().encode(source).length > 65536)
    throw RangeError("App plan exceeds 64 KiB");
  return Object.freeze(JSON.parse(source));
}
validateAppPlan.controlFactory = Object.freeze({
  topics: ["workflow", "product", "documents"],
  signature: "validateAppPlan(plan,{capabilities,files})",
  behavior:
    "Validates real requirements, window ownership, state-module/persistence boundaries, capability coverage, selected engine resource profiles and acceptance journeys before sdk.create writes anything. Creation stores rice-app-plan.json. The plan describes work; native interaction, saved-file checks and visual review supply completion evidence.",
});

import { Message } from "./messages.mjs";
/** Plain-text catalogs: translated content never becomes executable markup. */
export class Localizer {
  static controlFactory = Object.freeze({
    topics: ["localization", "forms", "product"],
    constructor:
      'new Localizer({locale:"en",fallback:"en",catalogs:{en:{save:"Save {name}"}}})',
    methods:
      "text(key,values), number(value,options?), date(value,options?), plural(value,{one,other,...}), setLocale(locale), bind(control,property,key,values?), subscribe(listener). Uses Intl; bindings release with native controls.",
  });
  constructor({ locale = "en", fallback = "en", catalogs = {} } = {}) {
    this.catalogs = structuredClone(catalogs);
    this.fallback = Intl.getCanonicalLocales(fallback)[0];
    this.locale = Intl.getCanonicalLocales(locale)[0];
    this.listeners = new Set();
    if (!this.locale || !this.fallback || !this.catalogs[this.fallback])
      throw TypeError("A fallback catalog is required");
    for (const catalog of Object.values(this.catalogs)) {
      if (
        !catalog ||
        typeof catalog !== "object" ||
        Array.isArray(catalog) ||
        Object.keys(catalog).length > 4096 ||
        Object.values(catalog).some(
          (v) => typeof v !== "string" || v.length > 16384,
        )
      )
        throw TypeError("Invalid message catalog");
    }
  }
  text(key, values = {}) {
    const catalog =
        this.catalogs[this.locale] || this.catalogs[this.locale.split("-")[0]],
      template = catalog?.[key] ?? this.catalogs[this.fallback]?.[key];
    if (typeof template !== "string")
      throw Error("Missing translated message: " + key);
    return template.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (_, id) => {
      if (
        !Object.hasOwn(values, id) ||
        !["number", "string"].includes(typeof values[id])
      )
        throw TypeError("Missing plain-text message value: " + id);
      return String(values[id]);
    });
  }
  number(value, options) {
    return new Intl.NumberFormat(this.locale, options).format(value);
  }
  date(value, options) {
    return new Intl.DateTimeFormat(this.locale, options).format(value);
  }
  plural(value, forms) {
    const key = new Intl.PluralRules(this.locale).select(value);
    if (typeof (forms[key] ?? forms.other) !== "string")
      throw TypeError("Plural forms require other");
    return this.text(forms[key] ?? forms.other, { count: this.number(value) });
  }
  setLocale(locale) {
    const next = Intl.getCanonicalLocales(locale)[0];
    if (!next) throw TypeError("Locale is required");
    this.locale = next;
    for (const listener of [...this.listeners]) listener(next);
  }
  subscribe(listener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  bind(control, property, key, values = {}) {
    const refresh = () =>
      control.setProperty(
        property,
        this.text(key, typeof values === "function" ? values() : values),
      );
    refresh();
    const stop = this.subscribe(refresh),
      destroy = control.on(Message.DESTROY, () => stop());
    return () => {
      stop();
      destroy();
    };
  }
}

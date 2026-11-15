/** Typed object/document clipboard with an interoperable plain-text companion. */
export class Clipboard {
  static controlFactory = Object.freeze({
    topics: ["documents", "visual", "workflow"],
    constructor: "new Clipboard({services})",
    methods:
      "read(acceptedMimeTypes), write([{mimeType,data:Uint8Array}]), copyJson(mimeType,value,{text}), readJson(mimeType,validate). Declared clipboard.readFormats/writeFormats permissions are required. Copy has to succeed before an app commits Cut. JSON import validates before document mutation; never execute clipboard markup.",
  });
  constructor({ services } = {}) {
    if (!services?.clipboard)
      throw TypeError("Clipboard services are required");
    this.service = services.clipboard;
  }
  read(accepted) {
    return this.service.readFormats({ accepted });
  }
  async write(formats) {
    if ((await this.service.writeFormats({ formats })) !== true)
      throw Error("Clipboard write was not acknowledged");
    return true;
  }
  copyJson(mimeType, value, { text = "" } = {}) {
    if (
      typeof mimeType !== "string" ||
      !mimeType.startsWith("application/") ||
      mimeType.length > 255 ||
      typeof text !== "string"
    )
      throw TypeError("Choose an application MIME type and plain text");
    const encoder = new TextEncoder(),
      json = JSON.stringify(value);
    if (json === undefined) throw TypeError("Clipboard JSON is required");
    return this.write([
      { mimeType, data: encoder.encode(json) },
      { mimeType: "text/plain", data: encoder.encode(text) },
    ]);
  }
  async readJson(mimeType, validate) {
    if (typeof validate !== "function")
      throw TypeError("Clipboard import requires a validator");
    const value = await this.read([mimeType]);
    if (value === null) return null;
    if (
      value.mimeType !== mimeType ||
      !(value.data instanceof Uint8Array) ||
      value.data.length > 4 * 1024 * 1024
    )
      throw TypeError("Unexpected clipboard payload");
    return validate(
      JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(value.data)),
    );
  }
}

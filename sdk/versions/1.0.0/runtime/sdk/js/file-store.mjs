/** Bounded ranged I/O and host-owned atomic transactions. */
export class FileStore {
  static controlFactory = Object.freeze({
    topics: ["documents", "workflow", "files"],
    constructor: "new FileStore({services,maximumBytes:67108864})",
    methods:
      "read(file,{signal}), version(file), write(file,data,{expectedVersion,signal,requireDurable:true}), releaseVersion(version). file is {handle} or app-relative {path}; add version to read a captured immutable version. write returns {size,atomic,durable,version}; requireDurable:true rejects missing durability acknowledgement. An explicit false permits mounted providers to report durable:false.",
  });
  constructor({ services, maximumBytes = 64 * 1024 * 1024 } = {}) {
    if (
      !services?.files ||
      !Number.isSafeInteger(maximumBytes) ||
      maximumBytes < 1 ||
      maximumBytes > 64 * 1024 * 1024
    )
      throw TypeError("Invalid file store");
    this.files = services.files;
    this.maximumBytes = maximumBytes;
  }
  async read(file, { signal } = {}) {
    signal?.throwIfAborted();
    const stat = await this.files.stat(file);
    if (
      stat.kind !== 1 ||
      !Number.isSafeInteger(stat.size) ||
      stat.size < 0 ||
      stat.size > this.maximumBytes
    )
      throw RangeError("File exceeds read budget");
    const data = new Uint8Array(stat.size);
    for (let offset = 0; offset < data.length; offset += 65536) {
      signal?.throwIfAborted();
      const length = Math.min(65536, data.length - offset),
        chunk = await this.files.read({ ...file, offset, length });
      if (!(chunk instanceof Uint8Array) || chunk.length !== length)
        throw Error("Incomplete file read");
      data.set(chunk, offset);
    }
    signal?.throwIfAborted();
    return data;
  }
  version(file) {
    return this.files.version(file);
  }
  releaseVersion(version) {
    return this.files.releaseVersion({ version });
  }
  async write(
    file,
    data,
    { expectedVersion, signal, requireDurable = true } = {},
  ) {
    if (!(data instanceof Uint8Array) || data.length > this.maximumBytes)
      throw RangeError("File exceeds write budget");
    signal?.throwIfAborted();
    const begun = await this.files.beginWrite({
      ...file,
      size: data.length,
      requireDurable,
      ...(expectedVersion === undefined ? {} : { expectedVersion }),
    });
    let committed = false;
    try {
      const maximum = begun.maximumChunkBytes;
      if (
        typeof begun.transaction !== "string" ||
        !Number.isInteger(maximum) ||
        maximum < 1 ||
        maximum > 256 * 1024
      )
        throw Error("Invalid write transaction");
      for (let offset = 0; offset < data.length; offset += maximum) {
        signal?.throwIfAborted();
        const end = Math.min(offset + maximum, data.length);
        const acknowledged = await this.files.writeChunk({
          transaction: begun.transaction,
          offset,
          data: data.slice(offset, offset + maximum),
        });
        if (acknowledged !== end) throw Error("Incomplete transaction chunk");
      }
      signal?.throwIfAborted();
      const result = await this.files.commitWrite({
        transaction: begun.transaction,
      });
      committed = true;
      if (result?.size !== data.length || result.atomic !== true)
        throw Error("Incomplete atomic write");
      if (requireDurable && result.durable !== true)
        throw Error("Storage did not acknowledge a durable save");
      return result;
    } finally {
      if (!committed)
        await this.files
          .abortWrite({ transaction: begun.transaction })
          .catch(() => {});
    }
  }
}

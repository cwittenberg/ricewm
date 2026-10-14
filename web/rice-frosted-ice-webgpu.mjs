import { FROSTED_ICE_WGSL } from "./rice-frosted-ice-wgsl.mjs";

// Share compilation and the presenter's device; each material owns only its
// bounded backdrop, uniform buffer and output canvas. It never owns the device.
const pipelines = new WeakMap();
function pipelineFor(device, format) {
  let formats = pipelines.get(device);
  if (!formats) {
    formats = new Map();
    pipelines.set(device, formats);
  }
  if (!formats.has(format)) {
    const module = device.createShaderModule({ code: FROSTED_ICE_WGSL });
    formats.set(
      format,
      device.createRenderPipelineAsync({
        layout: "auto",
        vertex: {
          module,
          entryPoint: "vertex_main",
        },
        fragment: {
          module,
          entryPoint: "fragment_main",
          targets: [{ format }],
        },
        primitive: { topology: "triangle-list" },
      }),
    );
  }
  return formats.get(format);
}
export class RiceWebGpuGlass {
  constructor({
    device,
    format,
    canvasFactory,
    onReady = () => {},
    onFailure = () => {},
  }) {
    this.device = device;
    this.format = format;
    this.canvasFactory = canvasFactory;
    this.onFailure = onFailure;
    this.closed = false;
    this.failed = false;
    this.pipeline = null;
    this.canvas = null;
    this.context = null;
    this.bitmap = null;
    this.texture = null;
    this.uniform = null;
    this.width = 0;
    this.height = 0;
    this.revision = -1;
    this.drawKey = null;
    this.counters = {
      draws: 0,
      reuses: 0,
      uploads: 0,
      uploadBytes: 0,
      textureAllocations: 0,
      bufferResizes: 0,
    };
    this.ready = pipelineFor(device, format)
      .then((pipeline) => {
        if (!this.closed) {
          this.pipeline = pipeline;
          onReady();
        }
        return !this.closed;
      })
      .catch((error) => {
        this.fail(error);
        return false;
      });
    device.lost.then((info) => {
      if (!this.closed)
        this.fail(new Error(info.message || "WebGPU device lost"));
    });
  }
  fail(error) {
    if (this.closed || this.failed) return;
    this.failed = true;
    this.dispose();
    this.onFailure(error);
  }
  draw(context, capture, rect, radius, optics, now, sampleRect, revision) {
    if (this.closed || !this.pipeline) return false;
    try {
      const device = this.device,
        limit = device.limits.maxTextureDimension2D;
      if (
        Math.max(capture.width, capture.height, rect.width, rect.height) > limit
      )
        throw Error("Glass surface exceeds WebGPU texture limit");
      if (!this.canvas) {
        this.canvas = this.canvasFactory(rect.width, rect.height);
        if (typeof this.canvas?.transferToImageBitmap !== "function")
          throw Error("WebGPU glass requires transferable canvas");
        this.context = this.canvas.getContext("webgpu");
        if (!this.context) throw Error("WebGPU glass canvas unavailable");
        this.context.configure({
          device,
          format: this.format,
          alphaMode: "premultiplied",
          usage:
            GPUTextureUsage.RENDER_ATTACHMENT |
            GPUTextureUsage.COPY_SRC |
            GPUTextureUsage.TEXTURE_BINDING,
        });
        this.uniform = device.createBuffer({
          size: 80,
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        this.sampler = device.createSampler({
          magFilter: "linear",
          minFilter: "linear",
        });
      }
      const width = Math.max(
        rect.width,
        Math.min(this.canvas.width, capture.width),
      );
      const height = Math.max(
        rect.height,
        Math.min(this.canvas.height, capture.height),
      );
      if (this.canvas.width !== width || this.canvas.height !== height) {
        this.canvas.width = width;
        this.canvas.height = height;
        this.drawKey = null;
        this.counters.bufferResizes++;
      }
      const key = JSON.stringify([
        revision,
        rect.width,
        rect.height,
        radius,
        sampleRect,
        optics,
        now,
      ]);
      if (this.drawKey === key) {
        this.counters.reuses++;
        context.drawImage(
          this.bitmap || this.canvas,
          0,
          0,
          rect.width,
          rect.height,
          rect.x,
          rect.y,
          rect.width,
          rect.height,
        );
        return true;
      }
      if (this.width !== capture.width || this.height !== capture.height) {
        this.texture?.destroy();
        this.width = capture.width;
        this.height = capture.height;
        this.texture = device.createTexture({
          size: [this.width, this.height],
          format: "rgba8unorm",
          usage:
            GPUTextureUsage.COPY_DST |
            GPUTextureUsage.TEXTURE_BINDING |
            GPUTextureUsage.RENDER_ATTACHMENT,
        });
        this.bindGroup = device.createBindGroup({
          layout: this.pipeline.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: this.texture.createView() },
            { binding: 1, resource: this.sampler },
            { binding: 2, resource: { buffer: this.uniform } },
          ],
        });
        this.revision = -1;
        this.counters.textureAllocations++;
      }
      if (this.revision !== revision) {
        device.queue.copyExternalImageToTexture(
          { source: capture },
          { texture: this.texture, premultipliedAlpha: false },
          [this.width, this.height],
        );
        this.revision = revision;
        this.counters.uploads++;
        this.counters.uploadBytes += this.width * this.height * 4;
      }
      device.queue.writeBuffer(
        this.uniform,
        0,
        new Float32Array([
          capture.width,
          capture.height,
          sampleRect.x,
          sampleRect.y,
          sampleRect.width,
          sampleRect.height,
          radius,
          optics.thickness,
          optics.opacity,
          optics.blur,
          optics.refraction,
          optics.chroma,
          optics.tintR,
          optics.tintG,
          optics.tintB,
          (Number(now) / 1000) % 10000,
          optics.lightX,
          optics.lightY,
          optics.pointerLight ? 1 : 0,
          optics.hyprglass ? 1 : 0,
        ]),
      );
      const encoder = device.createCommandEncoder();
      const pass = encoder.beginRenderPass({
        colorAttachments: [
          {
            view: this.context.getCurrentTexture().createView(),
            clearValue: { r: 0, g: 0, b: 0, a: 0 },
            loadOp: "clear",
            storeOp: "store",
          },
        ],
      });
      pass.setViewport(0, 0, rect.width, rect.height, 0, 1);
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.bindGroup);
      pass.draw(3);
      pass.end();
      device.queue.submit([encoder.finish()]);
      this.bitmap?.close();
      // Transfer keeps the completed GPU image alive for both clipped passes.
      // Direct drawImage(WebGPU canvas) is not reliable across browser backends.
      this.bitmap = this.canvas.transferToImageBitmap();
      this.drawKey = key;
      this.counters.draws++;
      context.drawImage(
        this.bitmap || this.canvas,
        0,
        0,
        rect.width,
        rect.height,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
      );
      return true;
    } catch (error) {
      this.fail(error);
      return false;
    }
  }
  memorySnapshot() {
    return {
      estimatedBytes:
        4 *
          ((this.canvas?.width || 0) * (this.canvas?.height || 0) +
            (this.bitmap?.width || 0) * (this.bitmap?.height || 0) +
            this.width * this.height) +
        (this.uniform ? 80 : 0),
      ...this.counters,
    };
  }
  dispose() {
    if (this.closed) return;
    this.closed = true;
    this.bitmap?.close();
    this.bitmap = null;
    this.texture?.destroy();
    this.uniform?.destroy();
    this.context?.unconfigure();
    if (this.canvas) {
      this.canvas.width = 1;
      this.canvas.height = 1;
    }
    this.canvas = null;
    this.context = null;
    this.texture = null;
    this.uniform = null;
    this.pipeline = null;
    this.bindGroup = null;
    this.width = 0;
    this.height = 0;
  }
}

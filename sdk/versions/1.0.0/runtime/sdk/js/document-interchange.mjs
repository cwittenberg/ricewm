import { openWasmDeckBackend } from './presentation/wasm-engine.mjs';
import { installShapeCatalog } from './presentation/shape-catalog.mjs';
import { validateVisualDocument, richTextPlainText } from './visual-document.mjs';
import { unionBounds, routeConnector, shapePath } from './visual-geometry.mjs';

/** Shared Office engine transport, also used by the native presentation editor. */
export function openPresentationEngine({ path = 'presentation.wasm', ...options } = {}) {
    return openWasmDeckBackend({ path, ...options });
}
openPresentationEngine.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature:
        'await openPresentationEngine({services,path:"presentation.wasm",newDocument:true,format:"pptx"|"odp",signal})',
    methods:
        'exchange(operation,arguments,expectedRevision), exportBytes(), close(); authoritative native engine, approved compute permissions, revision-checked mutations',
});

/** Export ordinary visual objects as editable presentation objects, not pixels. */
export async function exportVisualPresentation({
    document,
    services,
    path = 'presentation.wasm',
    format = 'pptx',
    width = 960,
    height = 540,
    signal,
    engine: provided,
} = {}) {
    const value = validateVisualDocument(document);
    if (
        !['pptx', 'odp'].includes(format) ||
        ![width, height].every((v) => Number.isFinite(v) && v > 0 && v <= 5376)
    )
        throw TypeError('Invalid presentation export format or page size');
    const unsupported = value.objects.filter(
        (o) =>
            o.shape === 'path' ||
            o.image ||
            o.richText?.some((block) =>
                block.runs.some((run) => run.bold || run.italic || run.underline || run.link),
            ),
    );
    if (unsupported.length)
        throw Error(
            'Presentation export cannot preserve custom paths, pictures or mixed rich runs: ' +
                unsupported.map((o) => o.id).join(', '),
        );
    const engine =
        provided ?? (await openPresentationEngine({ services, path, newDocument: true, format, signal }));
    try {
        const catalogue = installShapeCatalog(await engine.exchange('shape-catalog'));
        const projection = await engine.exchange('project'),
            slide = projection.result.slide.id;
        const apply = (commands) => engine.exchange('apply', { commands }, engine.revision);
        await apply([{ type: 'set-slide-size', width, height }]);
        const bounds = unionBounds(value.objects),
            scale = Math.min((width - 48) / bounds.width, (height - 48) / bounds.height),
            point = (p) => ({ x: 24 + (p.x - bounds.x) * scale, y: 24 + (p.y - bounds.y) * scale });
        const shapeIds = new Set(catalogue.map((s) => s.id)),
            byId = new Map(value.objects.map((o) => [o.id, o])),
            commands = [];
        for (const edge of value.connections) {
            const source = byId.get(edge.source),
                target = byId.get(edge.target),
                route = routeConnector(source, target, {
                    ...edge,
                    routing: edge.routing === 'orthogonal' ? 'orthogonal' : 'straight',
                    obstacles: value.objects,
                });
            for (let i = 1; i < route.points.length; i++)
                commands.push({
                    type: 'insert-line',
                    slide,
                    start: point(route.points[i - 1]),
                    end: point(route.points[i]),
                    stroke: { color: edge.color || '#77808d', width: 1.5 },
                });
        }
        for (const o of value.objects) {
            const p = point(o),
                shape = o.shape === 'pill' || o.shape === 'roundedRect' ? 'roundRect' : o.shape || 'rect',
                kind = ['rect', 'ellipse'].includes(shape) || shapeIds.has(shape) ? shape : 'rect';
            if (kind !== shape) throw Error('Presentation engine lacks requested shape ' + shape);
            const box = { ...p, width: o.width * scale, height: o.height * scale };
            commands.push({
                type: 'insert-object',
                slide,
                kind,
                box,
                fill: o.style?.fill || '#ffffff',
                stroke: { color: o.style?.stroke || '#77808d', width: (o.style?.strokeWidth ?? 1) * scale },
                source: o,
            });
            const text = o.richText ? richTextPlainText(o.richText) : o.text || '';
            if (text)
                commands.push({
                    type: 'insert-object',
                    slide,
                    kind: 'text',
                    box,
                    text,
                    fill: null,
                    stroke: null,
                    source: o,
                });
        }
        for (let start = 0; start < commands.length; start += 32) {
            const batch = commands.slice(start, start + 32),
                reply = await apply(batch.map(({ source, ...command }) => command)),
                styles = [];
            for (const [index, command] of batch.entries()) {
                const source = command.source;
                if (!source) continue;
                const created = reply.result.created[index];
                if (!created?.object) throw Error('Presentation engine omitted a created object identity');
                if (source.rotation)
                    styles.push({
                        type: 'set-object-rotation',
                        slide,
                        object: created.object,
                        degrees: source.rotation,
                    });
                if (command.kind === 'text')
                    styles.push({
                        type: 'set-text-style',
                        slide,
                        object: created.object,
                        style: {
                            color: source.style?.foreground || '#20242d',
                            fontFamily: 'Liberation Sans',
                            fontSize: Math.max(1, Math.min(512, (source.style?.fontSize || 16) * scale)),
                            align: 'center',
                        },
                    });
            }
            for (let i = 0; i < styles.length; i += 32) await apply(styles.slice(i, i + 32));
        }
        const result = await engine.exportBytes();
        return {
            ...result,
            format,
            mime:
                format === 'pptx'
                    ? 'application/vnd.openxmlformats-officedocument.presentationml.presentation'
                    : 'application/vnd.oasis.opendocument.presentation',
            objectCount: value.objects.length,
            connectionCount: value.connections.length,
        };
    } finally {
        if (!provided) await engine.close();
    }
}
exportVisualPresentation.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature:
        'await exportVisualPresentation({document,services,path:"presentation.wasm",format:"pptx"|"odp",width:960,height:540,signal})',
    behavior:
        'Exports a complete page of editable shapes, text and connections through the shared native Office engine. Returns bytes, mime and counts. Unsupported fidelity rejects explicitly; save bytes using granted durable file services. No application launch or private Bus access.',
});

const xml = (text) =>
    String(text).replace(
        /[&<>"']/g,
        (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c],
    );
export function exportVisualSvg(document) {
    const value = validateVisualDocument(document),
        bounds = unionBounds(value.objects),
        byId = new Map(value.objects.map((o) => [o.id, o]));
    const edges = value.connections
        .map((edge) => {
            const route = routeConnector(byId.get(edge.source), byId.get(edge.target), edge);
            return `<path d="${route.path.map((s) => s.join(' ')).join(' ')}" fill="none" stroke="${xml(edge.color || '#77808d')}"/>`;
        })
        .join('');
    const shapes = value.objects
        .map((o) => {
            const fill = xml(o.style?.fill || '#ffffff'),
                stroke = xml(o.style?.stroke || '#77808d'),
                shape =
                    o.shape === 'ellipse'
                        ? `<ellipse cx="${o.x + o.width / 2}" cy="${o.y + o.height / 2}" rx="${o.width / 2}" ry="${o.height / 2}"`
                        : `<rect x="${o.x}" y="${o.y}" width="${o.width}" height="${o.height}" rx="${o.shape === 'pill' ? o.height / 2 : o.shape === 'roundedRect' ? 12 : 0}"`;
            if (o.image) throw Error('SVG export cannot preserve this embedded image: ' + o.id);
            const geometry =
                o.shape === 'ellipse'
                    ? shape
                    : `<path d="${shapePath(o)
                          .map((s) => s.join(' '))
                          .join(' ')}"`;
            return `<g transform="rotate(${o.rotation || 0} ${o.x + o.width / 2} ${o.y + o.height / 2})">${geometry} fill="${fill}" stroke="${stroke}"/><text x="${o.x + o.width / 2}" y="${o.y + o.height / 2}" dominant-baseline="middle" text-anchor="middle" fill="${xml(o.style?.foreground || '#20242d')}">${xml(o.richText ? richTextPlainText(o.richText) : o.text || '')}</text></g>`;
        })
        .join('');
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${bounds.x - 16} ${bounds.y - 16} ${bounds.width + 32} ${bounds.height + 32}">${edges}${shapes}</svg>`;
}

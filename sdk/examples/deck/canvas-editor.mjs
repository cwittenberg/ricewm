import { hyperlinkAt } from './hyperlink-hit.mjs';
import { linkIdentity } from './hyperlink-contract.mjs';
import { canReorderObjects, objectOrderingActions } from './object-ordering.mjs';
import { tableCellAt } from './table-render.mjs';
import { tableDividerAt, resizedTableTracks, tableWithTracks, tableLocalPoint } from './table-geometry.mjs';
import { cellKey } from './table-values.mjs';
import { FormatPainterSession, Message } from '@ricewm/app';
import { captureObjectFormat, paintObjectFormat } from './format-painter.mjs';
import { geometryFrame } from './scene-preview.mjs';
import { alignmentCommands, objectAt, objectCorners, resizedBox, linePoints, translatedCommand, rotationHandle, draggedRotation, normalizedDegrees, normalizedSelection, canResize } from './canvas-geometry.mjs';

import { createFreeformDrawing } from './freeform-drawing.mjs';
import { drawingPoints, drawingBox, drawingCommand, drawingPreview } from './drawing-geometry.mjs';

/** Selection/gesture state is disposable. The injected command port remains
 * the only owner of document state, history, and persistence. */
export function createCanvasEditor({ surface, apply, changed, editText, notice, beforePointer, renderPreview, beginTableInput, tableInputKey, openHyperlink }) {
    let locked = false, tool = null, drawing = null, freeform = null, tableCell = null;
    const painter = new FormatPainterSession();
    let painterEpoch = 0;
    let pendingTextInput = null, textHandoff = null, pendingTextCreation = null;
    let scene = null, selected = [], drag = null, busy = false, ratio = 1, textFrames = new Map(), imageFrames = new Map();
    let editingCell = null, viewScale = 1, hoverCursor = 'default', previewWork = null, previewRequest = null, previewFrames = null;
    let pointerGate = null, pendingPointers = [], linkPress = null;
    const linkAt = point => hyperlinkAt({ scene, frame: scene.size, point: { x: point[0], y: point[1] }, textFrames });
    const objects = () => scene?.objects.filter(object => selected.includes(object.id)) || [];
    function repaint(boxes = new Map(), lines = new Map(), rotations = new Map()) {
        surface.setProperty('cursor', tool || painter.active ? 'crosshair' : hoverCursor);
        if (!scene?.size) return;
        const shown = drag?.table && drag.tracks ? {...scene,objects:scene.objects.map(o=>o.id===drag.table.id?tableWithTracks(drag.table,drag.tracks):o)} : scene;
        const frame = geometryFrame(shown, { selected, boxes, lines, rotations, textFrames: previewFrames || textFrames, imageFrames, placeholderGuides: true, editingCell, selectedCell: selectedTableCell(), preview: freeform?.preview() || (drawing ? drawingPreview(tool, drawing.points) : null) });
        ratio = frame.ratio; surface.submit(frame.frame);
    }
    function reflowPreview() {
        if(!renderPreview || !drag?.table || !drag.tracks) return;
        previewRequest={gesture:drag,tracks:drag.tracks};
        if(previewWork) return;
        previewWork=(async()=>{
            while(previewRequest) {
                const request=previewRequest;previewRequest=null;
                const snapshot={...scene,objects:scene.objects.map(o=>o.id===request.gesture.table.id?tableWithTracks(request.gesture.table,request.tracks):o)};
                try {
                    const result=await renderPreview(snapshot);
                    if(drag===request.gesture && drag.tracks===request.tracks) { previewFrames=result.frames;repaint(); }
                } catch { /* Keep geometry responsive; authoritative release repaint follows. */ }
            }
        })().finally(()=>{previewWork=null;});
    }
    function selection(ids) {
        selected = normalizedSelection(scene, ids);
        if (selected.length !== 1 || selected[0] !== tableCell?.object) tableCell = null;
        repaint(); changed?.(objects());
    }
    function selectedTableCell() {
        if (!tableCell || selected.length !== 1) return null;
        const object = objects()[0];
        if (!object?.table || object.id !== tableCell.object) return null;
        const cell = object.table.cells?.find(cell => cell.row === tableCell.row && cell.column === tableCell.column && !cell.covered);
        return cell ? { object, row: cell.row, column: cell.column, cell } : null;
    }
    function selectCell(object, cell) {
        selected = [object.id]; tableCell = { object: object.id, row: cell.row, column: cell.column };
        repaint(); changed?.(objects());
    }
    function cellText(keys = [], pointer = null) {
        const active = selectedTableCell();
        if (!active?.cell.editable) return;
        textHandoff = { ...active.object, id: cellKey(active.object.id, active.row, active.column), tableCell: active };
        editText?.(textHandoff, keys, pointer);
    }
    function nextTableCell(backwards = false) {
            const active = selectedTableCell(); if (!active) return;
            const cells = active.object.table.cells.filter(cell => !cell.covered);
            const index = cells.findIndex(cell => cell.row === active.row && cell.column === active.column);
            const next = cells[index + (backwards ? -1 : 1)];
            if (next) { selectCell(active.object, next); cellText(); }
    }
    function cancelFormatPainter() {
        if (!painter.active) return false;
        painter.cancel(); painterEpoch += 1; repaint(); changed?.(objects());
        notice?.('Format Painter cancelled.'); return true;
    }
    async function commit(commands) {
        if (locked || busy || !scene || !commands.length) return;
        busy = true; changed?.(objects());
        try { if(previewWork) await previewWork;return await apply(commands); }
        finally { busy = false; changed?.(objects()); }
    }
    function finishFreeform(result) {
        if (!result) { notice?.('Add more points before finishing the shape.'); return; }
        const command = { type: 'insert-freeform', slide: scene.slideId, ...result };
        tool = null; drawing = null; freeform = null; repaint();
        const input = result.closed ? { keys: [], bytes: 0, cancelled: false } : null;
        pendingTextInput = input;
        const creation = commit([command]).then(reply => {
            if (pendingTextInput === input) pendingTextInput = null;
            const object = objects()[0];
            if (input && !input.cancelled && reply?.result?.created?.some(value => value.object === object?.id) &&
                input.keys.some(key => key.text && !(key.modifiers & 12) || key.keyCode === 36 || key.key === 'Enter')) return beginText(input.keys);
        }).catch(error => {
            if (pendingTextInput === input) pendingTextInput = null;
            notice?.(`Cannot create shape: ${error.message}`);
        }).finally(() => { if (pendingTextCreation === creation) pendingTextCreation = null; });
        if (input) pendingTextCreation = creation;
    }
    function handlePointer(event) {
        if (!scene?.size || busy || locked) return;
        const detail = event.detail, point = [detail.x / ratio, detail.y / ratio];
        if (!point.every(Number.isFinite)) return;
        if (linkPress) {
            const pressed = linkPress;
            if (detail.kind === 'cancel' || Math.hypot(detail.x - pressed.x, detail.y - pressed.y) * viewScale > 5) pressed.cancelled = true;
            if (detail.kind === 'up' || detail.kind === 'cancel') {
                linkPress = null;
                if (detail.kind === 'up' && [0,1].includes(detail.button) && !pressed.cancelled && linkIdentity(linkAt(point)) === linkIdentity(pressed.link)) void openHyperlink?.(pressed.link);
            }
            event.preventDefault(); return;
        }
        if (!tool && !painter.active && detail.kind === 'down' && [0,1].includes(detail.button) && (detail.modifiers & 4) && !(detail.modifiers & 8)) {
            const link = linkAt(point);
            if (link && openHyperlink) { linkPress = { x: detail.x, y: detail.y, link }; event.preventDefault(); return; }
        }
        // Focus can move into the native cell editor before the originating
        // release reaches this surface. An unheld hover must end that handoff
        // and resume divider hit testing, rather than create a text selection.
        if(textHandoff?.tableCell && (detail.kind==='up' || detail.kind==='move' && (detail.modifiers&0x100))) {
            editText?.(textHandoff,[],{point:tableLocalPoint(textHandoff.tableCell.object,point),phase:detail.kind,extend:Boolean(detail.modifiers&1)});
            if(detail.kind==='up')textHandoff=null;
            return;
        }
        textHandoff = null;
        if (detail.kind === 'cancel') { drag = null; previewRequest=null;previewFrames=null;drawing = null; freeform = null; tool = null; repaint(); return; }
        if (detail.kind === 'down' && detail.button === 3) {
            cancelFormatPainter();
            tool = null; drawing = null; freeform = null; drag = null;
            const object = objectAt(scene, ...point, 6 / ratio);
            if (!object || !selected.includes(object.id)) selection(object ? [object.id] : []);
            else repaint();
            return;
        }
        if (painter.active) {
            if (detail.kind === 'down' && [0, 1].includes(detail.button)) {
                const target = objectAt(scene, ...point, 6 / ratio);
                if (!target) return;
                const epoch = painterEpoch;
                void painter.apply(async snapshot => {
                    const reply = await commit(paintObjectFormat(snapshot, target, scene.slideId));
                    if (!reply?.result) throw new Error('Formatting was not applied. The copied format is retained.');
                })
                    .then(() => {
                        selection([target.id]);
                        notice?.(epoch !== painterEpoch ? 'Format Painter cancelled.' : painter.active ? 'Format Painter: choose another target. Escape cancels.' : 'Formatting applied.');
                    })
                    .catch(error => notice?.(error.message));
            }
            return;
        }
        if (freeform) {
            try {
                freeform.move(point, Boolean(detail.modifiers & 1));
                if (detail.kind === 'down' && [0, 1].includes(detail.button)) {
                    const result = freeform.click(freeform.nextPoint || point, ratio);
                    if (result) { finishFreeform(result); return; }
                }
                repaint();
            } catch (error) { notice?.(error.message); }
            return;
        }
        if (tool) {
            if (detail.kind === 'down' && [0, 1].includes(detail.button)) {
                if (drawing?.waiting) drawing.finishing = true;
                else drawing = { start: point, points: drawingPoints(point, point, tool) };
            }
            if (drawing && ['move', 'up', 'down'].includes(detail.kind)) {
                drawing.points = drawingPoints(drawing.start, point, tool, Boolean(detail.modifiers & 1));
                repaint();
                if (detail.kind === 'up') {
                    const { start, end } = drawing.points, box = drawingBox(drawing.points);
                    const enough = tool === 'line' ? Math.hypot(end.x - start.x, end.y - start.y) * ratio >= 3 : Math.min(box.width, box.height) * ratio >= 3;
                    if (enough) {
                        const command = drawingCommand(scene.slideId, tool, drawing.points);
                        const input = command.type !== 'insert-line' ? { keys: [], bytes: 0, cancelled: false } : null;
                        pendingTextInput = input;
                        tool = null; drawing = null; freeform = null; repaint();
                        const creation = commit([command]).then(reply => {
                            if (pendingTextInput === input) pendingTextInput = null;
                            const object = objects()[0];
                            if (input && !input.cancelled && reply?.result?.created?.some(value => value.object === object?.id)) {
                                if (command.kind === 'text') {
                                    textHandoff = object;
                                    editText?.(object, input.keys);
                                } else if (input.keys.some(key => (key.text && !(key.modifiers & 12)) || key.keyCode === 36 || key.key === 'Enter')) {
                                    return beginText(input.keys);
                                }
                            }
                        }).catch(error => {
                            if (pendingTextInput === input) pendingTextInput = null;
                            notice?.(`Cannot create object: ${error.message}`);
                        }).finally(() => {
                            if (pendingTextCreation === creation) pendingTextCreation = null;
                        });
                        if (input) pendingTextCreation = creation;
                    } else if (drawing.finishing) { drawing = null; freeform = null; repaint(); }
                    else drawing.waiting = true;
                }
            }
            return;
        }
        if (detail.kind === 'down' && [0, 1].includes(detail.button)) {
            const existing = objects();
            const rotate = existing.length === 1 ? rotationHandle(existing[0], ratio) : null;
            if (rotate && Math.hypot(rotate.point[0]-point[0], rotate.point[1]-point[1])*ratio < 10) {
                drag = { point, values: existing, rotating: true, rotations: new Map() }; return;
            }
            const handle = existing.length === 1 && canResize(existing[0])
                ? objectCorners(existing[0]).findIndex(corner => Math.hypot(corner[0] - point[0], corner[1] - point[1]) * ratio < 10) : -1;
            const object = handle >= 0 ? existing[0] : objectAt(scene, ...point, 6 / ratio), additive = Boolean(detail.modifiers & 5);
            if (!object) { selection([]); drag = null; return; }
            if (additive) { selection(selected.includes(object.id) ? selected.filter(id => id !== object.id) : [...selected, object.id]); return; }
            if (!selected.includes(object.id)) selection([object.id]);
            const divider=handle<0?tableDividerAt(object,point,5/(ratio*viewScale)):null;
            if(divider) {
                editingCell=null;previewFrames=null;
                drag={table:object,divider,tracks:null};hoverCursor=divider.axis==='column'?'col-resize':'row-resize';
                surface.setProperty('focused',true);repaint();return;
            }
            const cell = handle < 0 ? tableCellAt(object, point, 4 / ratio) : null;
            if (cell) {
                selectCell(object, cell);cellText([], {point:tableLocalPoint(object,point),phase:'down',extend:Boolean(detail.modifiers&1)});drag = null;return;
            }
            tableCell = null;
            const values = objects();
            if (values.some(item => !item.edit?.canTransform)) return;
            const resize = handle;
            drag = { point, values, resize, boxes: new Map(), lines: new Map() };
        } else if (detail.kind === 'move' && drag) {
            if(drag.table) {drag.tracks=resizedTableTracks(drag.table,drag.divider,point);repaint();reflowPreview();return;}
            if (drag.rotating) {
                drag.rotations.set(drag.values[0].id, draggedRotation(drag.values[0], drag.point, point, Boolean(detail.modifiers & 1)));
                repaint(new Map(), new Map(), drag.rotations); return;
            }
            const dx = point[0] - drag.point[0], dy = point[1] - drag.point[1];
            for (const object of drag.values) {
                if (object.geometry === 'line' && object.line?.editable) {
                    const points = linePoints(object).map(([x, y], index) => drag.resize < 0 || drag.resize === index ? { x: x + dx, y: y + dy } : { x, y });
                    drag.lines.set(object.id, { start: points[0], end: points[1], editable: true }); continue;
                }
                const box = object.edit.box;
                drag.boxes.set(object.id, drag.resize >= 0
                    ? resizedBox(object, dx, dy, drag.resize)
                    : { ...box, x: box.x + dx, y: box.y + dy });
            }
            repaint(drag.boxes, drag.lines);
        } else if (detail.kind === 'up' && drag) {
            const transaction = drag; drag = null;
            if(transaction.table) {
                previewRequest=null;previewFrames=null;hoverCursor='default';
                if(transaction.tracks && ['columnWidths','rowHeights'].some(key=>transaction.tracks[key].some((v,i)=>Math.abs(v-transaction.table.table[key][i])>.01)))
                    void commit([{type:'resize-table',slide:scene.slideId,object:transaction.table.id,...transaction.tracks}]);
                else repaint();
                return;
            }
            if (transaction.rotating) {
                const degrees = transaction.rotations.get(transaction.values[0].id);
                if (degrees !== undefined && Math.abs(degrees - (transaction.values[0].edit.rotationDegrees || 0)) > 0.001) {
                    void commit([{ type: 'set-object-rotation', slide: scene.slideId, object: transaction.values[0].id, degrees }]);
                } else repaint();
                return;
            }
            const commands = [...transaction.boxes].filter(([id, box]) => {
                const before = transaction.values.find(object => object.id === id).edit.box;
                return Object.keys(box).some(key => Math.abs(box[key] - before[key]) > 0.01);
            }).map(([object, box]) => ({ type: 'set-object-box', slide: scene.slideId, object, box }));
            for (const [object, points] of transaction.lines) {
                const before = transaction.values.find(value => value.id === object).line;
                if (Math.hypot(points.end.x - points.start.x, points.end.y - points.start.y) < 0.01) continue;
                if (['start', 'end'].some(key => Math.hypot(points[key].x - before[key].x, points[key].y - before[key].y) > 0.01)) {
                    commands.push({ type: 'set-line-points', slide: scene.slideId, object, start: points.start, end: points.end });
                }
            }
            if (commands.length) void commit(commands); else repaint();
        } else if(detail.kind==='move') {
            const object=objectAt(scene,...point,6/(ratio*viewScale));
            const divider=tableDividerAt(object,point,5/(ratio*viewScale));
            hoverCursor=divider?(divider.axis==='column'?'col-resize':'row-resize'):'default';
            const link = linkAt(point);
            if (link && !divider) hoverCursor = 'pointer';
            surface.setProperty('cursor',hoverCursor);
            surface.setProperty('tooltip', link ? `Ctrl+click to open ${link.target.kind === 'slide' ? `slide ${link.slideIndex + 1}` : link.target.value}` : '');
        }
    }
    surface.on(Message.POINTER,event=>{
        if(pointerGate) {pendingPointers.push(event);return;}
        const prerequisite=event.detail?.kind==='down'?beforePointer?.(event.detail):null;
        if(prerequisite?.then) {
            pendingPointers.push(event);
            pointerGate=Promise.resolve(prerequisite).then(ok=>{
                const events=pendingPointers;pendingPointers=[];pointerGate=null;
                if(ok!==false) for(const queued of events)handlePointer(queued);
            }).catch(error=>{pendingPointers=[];pointerGate=null;notice?.(error.message);});
        } else handlePointer(event);
    });
    // Menu activation and the canvas focus patch cross different native turns.
    // The owning shell forwards keys still addressed to the menu while an
    // insertion is pending, so typing cannot lose its initial characters.
    function capturePendingText(event) {
        const key = event.detail;
        if (!pendingTextInput || !key?.pressed || event.defaultPrevented) return false;
        if (key.keyCode === 9 || key.key === 'Escape') pendingTextInput.cancelled = true;
        else if (!pendingTextInput.cancelled) {
            if(pendingTextInput.live) {tableInputKey?.({...key});event.preventDefault();return true;}
            const bytes = new TextEncoder().encode(key.text || '').length;
            if (pendingTextInput.keys.length < 512 && pendingTextInput.bytes + bytes <= 16000) {
                pendingTextInput.keys.push({ ...key }); pendingTextInput.bytes += bytes;
            } else notice?.('Text input exceeds the text box limit. Wait for the box to finish opening.');
        }
        event.preventDefault(); return true;
    }
    surface.on(Message.KEY, event => {
        const key = event.detail;
        if (!key?.pressed) return;
        if(drag?.table && (key.key==='Escape'||key.keyCode===9)) {
            drag=null;previewRequest=null;previewFrames=null;hoverCursor='default';repaint();event.preventDefault();return;
        }
        if (painter.active && (key.key === 'Escape' || key.keyCode === 9)) {
            cancelFormatPainter(); event.preventDefault(); return;
        }
        if (capturePendingText(event)) return;
        // Native focus travels back through the host asynchronously. Preserve
        // keys arriving on the canvas between creation and editor focus ack.
        if (textHandoff) { editText?.(textHandoff, [{ ...key }]); event.preventDefault(); return; }
        if (busy || locked) return;
        if (freeform) {
            if (key.key === 'Escape' || key.keyCode === 9) { tool = null; freeform = null; repaint(); }
            else if (key.key === 'Backspace' || key.keyCode === 22) { freeform.backspace(); repaint(); }
            else if (key.key === 'Enter' || key.keyCode === 36) finishFreeform(freeform.finish());
            event.preventDefault(); return;
        }
        if (selectedTableCell() && (key.key === 'Tab' || key.keyCode === 23)) {
            nextTableCell(Boolean(key.modifiers & 1)); event.preventDefault(); return;
        }
        if (selectedTableCell() && (key.text && !(key.modifiers & 12) || key.key === 'Enter' || key.keyCode === 36)) {
            cellText(key.text ? [{ ...key }] : []); event.preventDefault(); return;
        }
        if (key.text && !(key.modifiers & 12) && objects().length === 1 && (objects()[0].text?.editable || objects()[0].edit?.canAddText)) {
            void beginText([{ ...key }]); event.preventDefault(); return;
        }
        if (key.key === 'Escape' || key.keyCode === 9) { drag = null; tool = null; drawing = null; freeform = null; selection([]); event.preventDefault(); }
        if (key.key === 'Delete' || key.keyCode === 119) { void remove(); event.preventDefault(); }
        if (key.key === 'Enter' || key.keyCode === 36) { void beginText(); event.preventDefault(); }
        const arrows = { 111: [0, -1], 113: [-1, 0], 114: [1, 0], 116: [0, 1] };
        const delta = arrows[key.keyCode];
        if (delta && objects().every(object => object.edit?.canTransform)) {
            const step = key.modifiers & 1 ? 10 : 1;
            void commit(objects().map(object => translatedCommand(scene, object, delta[0] * step, delta[1] * step)));
            event.preventDefault();
        }
    });
    surface.on(Message.BLUR, () => { textHandoff = null; });
    async function beginText(keys = []) {
        if (selectedTableCell()) { cellText(keys); return; }
        if (busy || locked || objects().length !== 1) return;
        const object = objects()[0];
        if (object.text?.editable) {
            textHandoff = object; editText?.(object, keys); return;
        }
        if (!object.edit?.canAddText) return;
        const input = { keys: [...keys], bytes: keys.reduce((sum, key) => sum + new TextEncoder().encode(key.text || '').length, 0), cancelled: false };
        pendingTextInput = input;
        const creation = commit([{ type: 'set-object-text', slide: scene.slideId, object: object.id, text: '' }]).then(() => {
            const target = objects().find(item => item.id === object.id);
            if (!input.cancelled && target?.text?.editable) {
                textHandoff = target; editText?.(target, input.keys);
            }
        }).catch(error => notice?.(`Cannot add text: ${error.message}`)).finally(() => {
            if (pendingTextInput === input) pendingTextInput = null;
            if (pendingTextCreation === creation) pendingTextCreation = null;
        });
        pendingTextCreation = creation;
        await creation;
    }
    function remove() {
        if (objects().some(object => !object.edit?.canDelete)) return;
        return commit(objects().map(object => ({ type: 'delete-object', slide: scene.slideId, object: object.id })));
    }
    return Object.freeze({
        setEditingCell(id) { if(editingCell!==id) {editingCell=id;repaint();} },
        setViewScale(value) {if(Number.isFinite(value)&&value>0)viewScale=value;},
        capturePendingText,
        async flushPendingTextInput() { if (pendingTextCreation) await pendingTextCreation; },
        get formatPainter() { return { active: painter.active, persistent: painter.persistent }; },
        get supportsNormalLineSpacing() { return Boolean(scene?.slideId.startsWith('odp:')); },
        cancelFormatPainter,
        paintFormat() {
            if (locked || busy) return;
            try {
                painter.activate(painter.active ? painter.snapshot : captureObjectFormat(objects().length === 1 ? objects()[0] : null)); painterEpoch += 1;
                drag = null; drawing = null; freeform = null; tool = null; textHandoff = null;
                repaint(); changed?.(objects()); surface.setProperty('focused', true);
                notice?.(painter.active ? `${painter.persistent ? 'Format Painter: choose targets. Escape cancels.' : 'Format Painter: choose a target. Double-click the brush to keep painting.'}${painter.snapshot.limitation ? ` ${painter.snapshot.limitation}` : ''}` : 'Format Painter cancelled.');
            } catch (error) { notice?.(error.message); }
        },
        get selectedTableCell() { return selectedTableCell(); },
        selectTableCell(objectId, row, column) {
            const object = scene?.objects.find(object => object.id === objectId);
            const cell = object?.table?.cells?.find(cell => cell.row === row && cell.column === column && !cell.covered);
            if (cell) selectCell(object, cell);
        },
        nextTableCell,
        insertTable({ rows, columns }) {
            if (!scene?.size || locked || busy) return;
            const width = scene.size.width * 0.8, height = Math.min(scene.size.height * 0.7, Math.max(80, rows * 40));
            const input = { keys: [], bytes: 0, cancelled: false, live:Boolean(beginTableInput) };
            pendingTextInput = input;
            // The dismissed menu remains a key route while creation is pending.
            // Clear the old canvas request so the eventual cell editor owns
            // focus. Let the actor collect patches; flushing here would consume
            // them inside an input dispatch before its response is published.
            surface.setProperty('focused', false);
            const bounds={x:(scene.size.width-width)/2,y:(scene.size.height-height)/2,width,height};
            beginTableInput?.({bounds,size:scene.size,columns,rows});
            const creation = commit([{ type: 'insert-table', slide: scene.slideId, rows, columns,
                bounds }])
                .then(reply => {
                    if (!input.cancelled && reply?.result?.created?.some(value => value.object === selectedTableCell()?.object.id)) cellText(input.keys);
                    return reply;
                }).finally(() => {
                    if (pendingTextInput === input) pendingTextInput = null;
                    if (pendingTextCreation === creation) pendingTextCreation = null;
                });
            pendingTextCreation = creation;
            return creation;
        },
        tableCellStyle(style) {
            const active = selectedTableCell(); if (!active?.cell.editable) return;
            return commit([{ type: 'set-table-cell-style', slide: scene.slideId, object: active.object.id, row: active.row, column: active.column, style }]);
        },
        tableStyle(style) {
            const object = objects().length === 1 ? objects()[0] : null; if (!object?.table?.canEditCells) return;
            return commit([{ type: 'set-table-style', slide: scene.slideId, object: object.id, style }]);
        },
        tableStructure(type, at, count) {
            const object = objects().length === 1 ? objects()[0] : null; if (!object?.table?.canEditStructure) return;
            return commit([{ type, slide: scene.slideId, object: object.id, at, count }]);
        },
        get selected() { return objects(); }, get drawingTool() { return tool; }, get busy() { return busy; },
        repaint() { repaint(); },
        setLocked(value) { locked = Boolean(value); if (locked) { linkPress = null; drag = null; drawing = null; freeform = null; tool = null; surface.setProperty('cursor', 'default'); } },
        select: selection, beginText,
        selectRun(id) { const object = scene?.objects.find(object => object.textRunIds.includes(id)); if (object) selection([object.id]); },
        setScene(next, frames = new Map(), pictures = new Map()) {
            linkPress = null; scene = next; textFrames = frames; imageFrames = pictures; drag = null; drawing = null; freeform = null; tool = null;
            selected = normalizedSelection(next, selected);
            if (selected.length === 1) {
                const object = objects()[0];
                if (object?.table && !selectedTableCell()) {
                    const cell = object.table.cells?.find(cell => !cell.covered);
                    tableCell = cell ? { object: object.id, row: cell.row, column: cell.column } : null;
                }
            } else tableCell = null;
            repaint(); changed?.(objects());
        },
        created(reply) { const values = reply?.result?.created; if (Array.isArray(values) && values.length) selected = values.map(value => value.object); },
        insert(kind) {
            if (!scene?.size || locked || busy) return;
            cancelFormatPainter();
            drag = null; drawing = null; tool = kind;
            freeform = ['polygon', 'polyline'].includes(kind) ? createFreeformDrawing({ closed: kind === 'polygon' }) : null;
            selection([]); surface.setProperty('focused', true);
            if (freeform) notice?.('Click to add points. Enter or double-click finishes; Backspace removes a point; Escape cancels.');
        },
        reorder(direction) {
            const targets = objects();
            if (!canReorderObjects(targets) || !objectOrderingActions.some(action => action.direction === direction)) return;
            return commit([{ type: 'reorder-objects', slide: scene.slideId, objects: targets.map(object => object.id), direction }]);
        },
        duplicate() {
            const targets = objects();
            if (!targets.length || targets.some(object => !object.edit?.canTransform)) return;
            return commit([{ type: 'duplicate-objects', slide: scene.slideId, objects: targets.map(object => object.id), offset: { x: 16, y: 16 } }]);
        },
        group() {
            const targets = objects();
            if (targets.length < 2 || targets.some(object => !object.edit?.canGroup || object.parentId)) return;
            return commit([{ type: 'group-objects', slide: scene.slideId, objects: targets.map(object => object.id) }]);
        },
        ungroup() {
            const targets = objects();
            if (targets.length !== 1 || !targets[0].group?.canUngroup) return;
            return commit([{ type: 'ungroup-object', slide: scene.slideId, object: targets[0].id }]);
        },
        align(direction, reference) { return commit(alignmentCommands(scene, objects(), direction, reference)); },
        fill(fill) { return commit(objects().filter(object => object.edit?.canStyle && object.geometry !== 'line').map(object => ({ type: 'set-object-fill', slide: scene.slideId, object: object.id, fill }))); },
        stroke(stroke) { return commit(objects().filter(object => object.edit?.canStyle).map(object => ({ type: 'set-object-stroke', slide: scene.slideId, object: object.id, stroke }))); },
        shadow(shadow) {
            const targets = objects();
            if (!targets.length || targets.some(object => !object.edit?.canSetShadow)) return;
            return commit(targets.map(object => ({ type: 'set-object-shadow', slide: scene.slideId, object: object.id,
                shadow: typeof shadow === 'function' ? shadow(object) : shadow })));
        },
        strokeColor(color) {
            return commit(objects().filter(object => object.edit?.canStyle).map(object => ({ type: 'set-object-stroke', slide: scene.slideId, object: object.id,
                stroke: color === null ? null : { color, width: object.stroke?.width ?? 2 } })));
        },
        strokeWidth(width) {
            return commit(objects().filter(object => object.edit?.canStyle).map(object => ({ type: 'set-object-stroke', slide: scene.slideId, object: object.id,
                stroke: { color: object.stroke?.color ?? '#287AC7', width } })));
        },
        style(style) {
            const active = selectedTableCell();
            if (active?.cell.editable) return commit([{ type: 'set-table-cell-style', slide: scene.slideId, object: active.object.id, row: active.row, column: active.column, style: { text: style } }]);
            const targets = objects().filter(object => object.text != null);
            if (!targets.length) { notice?.('Select a text box to format its text.'); return; }
            return commit(targets.map(object => ({ type: 'set-text-style', slide: scene.slideId, object: object.id, style })));
        },
        textLayout(layout) {
            const targets = objects().filter(object => object.text?.editable && object.text.layout);
            return commit(targets.map(object => ({ type: 'set-text-layout', slide: scene.slideId, object: object.id, layout })));
        },
        paragraphSpacing(spacing) {
            const targets = objects();
            if (!targets.length || targets.some(object => !object.text?.editable)) return;
            return commit(targets.map(object => ({ type: 'set-paragraph-spacing', slide: scene.slideId, object: object.id, spacing })));
        },
        crop(crop) {
            const object = objects()[0];
            if (objects().length === 1 && object?.image?.canCrop) return commit([{ type: 'set-image-crop', slide: scene.slideId, object: object.id, crop }]);
        },
        rotate(degrees, relative = false) {
            const targets = objects();
            if (!Number.isFinite(degrees) || !targets.length || targets.some(object => !object.edit?.canRotate)) return;
            return commit(targets.map(object => ({ type: 'set-object-rotation', slide: scene.slideId, object: object.id,
                degrees: normalizedDegrees(degrees + (relative ? object.edit.rotationDegrees || 0 : 0)) })));
        },
        resize(box) {
            const object = objects()[0];
            if (objects().length === 1 && object?.edit?.canTransform && (canResize(object) || box.width === object.edit.box.width && box.height === object.edit.box.height)) return commit([{ type: 'set-object-box', slide: scene.slideId, object: object.id, box }]);
        },
        remove, cancel() {
            cancelFormatPainter();
            if (pendingTextInput) pendingTextInput.cancelled = true;
            textHandoff = null; drag = null; drawing = null; freeform = null; tool = null; repaint();
        },
    });
}

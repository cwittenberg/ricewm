import { Button, Column, DocumentViewport, Label, Message, SceneBuilder, SceneOverlay, SceneSurface, ScrollView, State, ZoomControl } from '@ricewm/app';

/** Alternate projections of the same document and the same retained notes editor. */
export function createWorkspaceViews({ port, notes, navigate, notice = () => {} }) {
    let current = null, mode = 'normal', previousNotes = false, closed = false, generation = 0, pageStart = 0, selectedId = null;
    const pageSize = 16;
    const entries = new Map();
    const outlineRows = new Column({ name: 'deck-outline-rows', text: 'panel:fixed:gap=4' });
    const previous = new Button({ name: 'outline-previous', text: 'Previous slides', enabled: false });
    const next = new Button({ name: 'outline-next', text: 'Next slides', enabled: false });
    const outline = new Column({ name: 'deck-outline-view', state: State.EXPANSIVE }, [new ScrollView({ state: State.EXPANSIVE }, [outlineRows]), previous, next]);
    async function turnPage(delta) {
        pageStart = Math.max(0, Math.min(Math.max(0, current.slides.length - 1), pageStart + delta * pageSize));
        try { await refresh(current); } catch (error) { if (!closed) notice(`Could not read outline: ${error.message}`); }
    }
    previous.on(Message.ACTIVATE, () => turnPage(-1)); next.on(Message.ACTIVATE, () => turnPage(1));
    const page = new SceneSurface({ name: 'notes-page-surface', presentation: 'page' });
    const noteOverlay = new SceneOverlay({ surface: page, bounds: { x: 72/612, y: .45, width: 468/612, height: .45 } });
    const notesViewport = new DocumentViewport({ name: 'notes-page-viewport', surface: page });
    notesViewport.clip.append(noteOverlay);
    const notesZoom = new ZoomControl({ name: 'notes-page-zoom', viewport: notesViewport, compact: true });
    notesZoom.visible = false;
    function mountNotesPage(open) {
        if (open) {
            previousNotes = notes.opened;
            notes.setOpen(true); notes.control.visible = false;
            notes.editor.moveTo(noteOverlay);
            // Keep the same input, draft and scalar selection. Document paint
            // removes form chrome instead of replacing the editor with a view.
            notes.editor.setProperty('documentFont', {family:'Inter',size_pt:10.5,weight:400,italic:false,underline:false});
            notes.editor.setProperty('documentPaint', {background:'#FFFFFF',foreground:'#1E1E1E'});
            notes.editor.setProperty('placeholder', 'Click to add text');
        } else {
            notes.editor.setProperty('documentFont', null);
            notes.editor.setProperty('documentPaint', null);
            notes.editor.setProperty('placeholder', null);
            notes.editor.moveTo(notes.editorHost);
            notes.setOpen(previousNotes);
        }
    }
    function notesPageFrame(frame) {
        const width = 612, height = 792, b = new SceneBuilder(width, height).rect([0, 0, width, height], 0xffffffff);
        if (frame) {
            const scale = Math.min(468 / frame.width, (height*.5-54) / frame.height),x=(width-frame.width*scale)/2,y=54,w=frame.width*scale,h=frame.height*scale;
            const notesTop=y+h+36;
            noteOverlay.setBounds({x:72/width,y:notesTop/height,width:468/width,height:(height-54-notesTop)/height});
            b.pushTransform([scale, 0, 0, scale, x, y]).append(frame.displayList).popTransform();
            b.line([x,y],[x+w,y],1,0xff808080).line([x+w,y],[x+w,y+h],1,0xff808080)
                .line([x+w,y+h],[x,y+h],1,0xff808080).line([x,y+h],[x,y],1,0xff808080);
        }
        page.submit(b.finish());
    }
    async function refresh(snapshot) {
        current = snapshot;
        if (mode === 'notesPage') notesPageFrame(snapshot.frame);
        if (mode !== 'outline' || !port?.outline) return;
        if (selectedId !== snapshot.selectedId) { const index = snapshot.slides.findIndex(item => item.id === snapshot.selectedId); if (index >= 0) pageStart = Math.floor(index / pageSize) * pageSize; selectedId = snapshot.selectedId; }
        if (pageStart >= snapshot.slides.length) pageStart = 0;
        const visible = snapshot.slides.slice(pageStart, pageStart + pageSize);
        previous.enabled = pageStart > 0; next.enabled = pageStart + pageSize < snapshot.slides.length;
        const epoch = ++generation, ids = new Set(visible.map(item => item.id));
        for (const [id, entry] of entries) if (!ids.has(id)) { outlineRows.remove(entry.control, { destroy: true }); entries.delete(id); }
        for (const slide of visible) {
            if (closed || epoch !== generation) return;
            let entry = entries.get(slide.id);
            if (!entry) {
                const title = new Button({ name: `outline-slide-${slide.index}`, accessibleName: `${slide.index + 1}. ${slide.name || 'Slide'}`, text: '', state: State.START_ALIGNED });
                const body = new Label({ text: '', state: State.MUTED });
                const control = new Column({ name: `outline-entry-${slide.index}`, text: 'panel:fixed:gap=2:multiline' }, [title, body]);
                entry = { control, title, body, revision: null, slide }; entries.set(slide.id, entry); outlineRows.append(control);
                title.on(Message.ACTIVATE, () => navigate(entry.slide.index, 0, entry.slide.id));
            }
            entry.slide = slide; entry.title.text = `${slide.index + 1}. ${slide.name || 'Slide'}`;
            entry.title.setProperty('selected', slide.id === snapshot.selectedId);
            if (entry.revision === snapshot.revision) continue;
            // Read bounded projection pages through the existing engine port.
            const reply = await port.project({ slide: slide.index, startRun: 0, limit: 32 });
            if (closed || epoch !== generation || reply.revision !== snapshot.revision) return;
            entry.body.text = (reply.result.slide?.text || []).map(run => run.text).join('\n').slice(0, 3500) || 'Empty slide';
            if (reply.result.nextRun !== null) entry.body.text += '\nMore text is available on the slide.';
            entry.revision = snapshot.revision;
        }
    }
    return Object.freeze({ outline, notesPage: notesViewport, notesZoom,
        async change(next, previous) {
            if (previous === 'notesPage') mountNotesPage(false);
            mode = next; generation++;
            if (next === 'notesPage') mountNotesPage(true);
            notesZoom.visible = next === 'notesPage';
            if (current) await refresh(current);
        }, refresh,
        close() { closed = true; generation++; },
    });
}

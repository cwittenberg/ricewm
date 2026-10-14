export function riceCompactFormFactor(viewport) {
    return viewport.width < 700 || (viewport.width <= 1366 &&
        (viewport.touch ?? globalThis.matchMedia?.('(pointer:coarse)').matches) === true);
}

/** Shared viewport adaptation for shell-owned DOM surfaces, including shadow roots.
 * Application colors, fonts and controls remain owned by their existing theme.
 */
export function adaptRiceDomSurface(root) {
    const doc = root.ownerDocument;
    const win = doc.defaultView;
    const host = root.host || doc.documentElement;
    const style = doc.createElement('style');
    style.textContent = `
:host([data-rice-compact-surface]) [role=dialog]{
 position:fixed!important;inset:var(--rice-surface-top) auto auto var(--rice-surface-left)!important;
 transform:translateX(-50%)!important;max-width:var(--rice-surface-width)!important;
 height:var(--rice-surface-height)!important;max-height:var(--rice-surface-height)!important;
 min-width:0!important;min-height:0!important;display:flex!important;flex-direction:column!important;
}
:host([data-rice-compact-surface]) [role=dialog]>:has(>[role=tablist]){
 display:flex!important;flex-direction:column!important;flex:1!important;min-height:0!important;overflow:hidden!important;
}
:host([data-rice-compact-surface]) [role=tablist]{
 flex-direction:row!important;flex-wrap:nowrap!important;justify-content:flex-start!important;align-items:center!important;
 flex:0 0 auto!important;overflow-x:auto!important;overflow-y:hidden!important;max-width:100%!important;min-height:0!important;
}
:host([data-rice-compact-surface]) [role=tablist]>[role=tab]{flex-shrink:0!important;margin-block:0!important}
:host([data-rice-compact-surface]) [role=tabpanel],
:host([data-rice-compact-surface]) [role=dialog] main{
 flex:1!important;min-height:0!important;min-width:0!important;max-height:none!important;overflow:auto!important;
}
:host([data-rice-compact-surface]) [role=dialog]>:first-child,
:host([data-rice-compact-surface]) [role=dialog]>[role=status]{flex-shrink:0!important}
`;
    root.append(style);
    const update = () => {
        const view = win.visualViewport;
        const width = view?.width || win.innerWidth;
        const height = view?.height || win.innerHeight;
        const compact = riceCompactFormFactor({width}) || height < 500;
        host.toggleAttribute('data-rice-compact-surface', compact);
        if (compact) {
            for (const [key, value] of Object.entries({left:(view?.offsetLeft || 0)+width/2, top:(view?.offsetTop || 0)+8, width:Math.max(1,width-16), height:Math.max(1,height-16)})) {
                host.style.setProperty(`--rice-surface-${key}`, `${value}px`);
            }
        } else for (const key of ['left','top','width','height']) host.style.removeProperty(`--rice-surface-${key}`);
    };
    win.addEventListener('resize', update);
    win.visualViewport?.addEventListener('resize', update);
    win.visualViewport?.addEventListener('scroll', update);
    update();
    return () => {
        win.removeEventListener('resize', update);
        win.visualViewport?.removeEventListener('resize', update);
        win.visualViewport?.removeEventListener('scroll', update);
        host.removeAttribute('data-rice-compact-surface');
        for (const key of ['left','top','width','height']) host.style.removeProperty(`--rice-surface-${key}`);
        style.remove();
    };
}

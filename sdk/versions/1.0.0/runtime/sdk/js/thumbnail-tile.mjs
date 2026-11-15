import { Column, Container, Label, Stack, Tooltip } from './controls.mjs';
import { Control, State } from './control.mjs';
import { Message } from './messages.mjs';
import { ReorderButton } from './reorder-controller.mjs';

/** Retained, native selectable preview. The caller owns content and selection. */
export class ThumbnailTile extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['content', 'number', 'title', 'numberWidth', 'buttonName', 'borderWidth', 'observeSize', 'numberPlacement', 'padding', 'aspectRatio', 'contentWidth']);
    static properties = Object.freeze({
        selected: { default: false, reflect: false, stateBit: null, validate: Boolean },
    });
    static authoring = Object.freeze({
        topics: Object.freeze(['visual','navigation','product','documents']),
        constructor: 'new ThumbnailTile({content:new SceneSurface({enabled:false}),number:"1",title:"Preview",numberWidth:24,borderWidth:3,selected:false})',
        methods: 'setNumber(text), setTitle(text), setSelected(boolean), setDropPosition(null|"before"|"after"), setSize(width,height), setLayout({numberPlacement:"leading"|"below",padding:{left,right,top,bottom},aspectRatio,contentWidth}), previewSize; button is a ReorderButton and owns native activation, pointer modifiers, keyboard and focus. Attach ReorderController to opt into dragging; use setDropPosition for a separate native insertion line. Content is retained and remains caller-owned. The number sits outside artwork. aspectRatio and contentWidth constrain the preview without stretching it. Native theme selection borders remain separate from preview paint. Virtualized lists use observeSize:false and setSize from one shared native size observer, preserving the native observer bound. The caller owns navigation, multi-selection, drag/drop, preview data and history.',
    });
    constructor({ content, number = '', title = '', numberWidth = 24, borderWidth = 3, buttonName = null, observeSize = true,
        numberPlacement = 'leading', padding = {}, aspectRatio = null, contentWidth = null, ...options } = {}) {
        if (!(content instanceof Control) || content.parent || typeof number !== 'string' || number.length > 32 ||
            typeof title !== 'string' || title.length > 512 || !Number.isInteger(numberWidth) || numberWidth < 16 || numberWidth > 128 ||
            !Number.isInteger(borderWidth) || borderWidth < 1 || borderWidth > 8 || typeof observeSize !== 'boolean' || buttonName !== null && typeof buttonName !== 'string') throw TypeError('Invalid thumbnail tile');
        super({ text: 'panel:flush:gap=0', state: State.EXPANSIVE, ...options });
        this.borderWidth = borderWidth;
        this.button = new ReorderButton({ name: buttonName || (this.name ? `${this.name}-button` : null), text: 'panel:flush:gap=0',
            accessibleName: options.accessibleName || title || number || 'Preview', enabled: this.enabled, state: State.FLAT | State.EXPANSIVE });
        this.number = new Label({ text: number, enabled: false, state: State.CENTER_ALIGNED });
        this.gutter = new Container({ text: 'positioned-item:0:0:0:1000', enabled: false, visible: Boolean(number), state: State.FLAT }, [this.number]);
        this.edges = Array.from({ length: 4 }, () => new Container({ enabled: false, state: State.FLAT }));
        this.border = new Stack({ text: 'panel:overlay:flush', enabled: false }, this.edges);
        this.body = new Stack({ text: 'panel:flush:gap=0', state: State.EXPANSIVE }, [content, this.border]);
        this.bodyFrame = new Container({ text: 'positioned-item:0:0:1000:1000', state: State.FLAT }, [this.body]);
        this.layoutRoot = this.button.append(new Stack({ text: 'panel:overlay:flush', state: State.EXPANSIVE }, [this.gutter, this.bodyFrame]));
        this.dropMarker=this.layoutRoot.append(new Container({text:'positioned-item:0:0:1:1000',enabled:false,visible:false,state:State.PRESSED|State.ACCENT}));
        this.numberWidth = numberWidth;
        if (observeSize) this.layoutRoot.on(Message.SIZE_CHANGED, event => {
            if (event.target !== this.layoutRoot || !(event.detail?.width > 0 && event.detail?.height > 0)) return;
            this.setSize(event.detail.width, event.detail.height);
        });
        this.tooltip = this.button.append(new Tooltip({ text: title, visible: Boolean(title) }));
        this.inputHost=this.append(new Stack({text:'panel:flush:gap=0',state:State.EXPANSIVE},[this.button]));
        this.setLayout({ numberPlacement, padding, aspectRatio, contentWidth });
        this.setTitle(title); this.updateSelection();
    }
    setProperty(name, value, options) {
        const changed = super.setProperty(name, value, options);
        if (this.edges && name === 'selected') this.updateSelection();
        if (this.button && name === 'enabled') this.button.enabled = this.enabled;
        return changed;
    }
    setNumber(text) {
        if (typeof text !== 'string' || text.length > 32) throw TypeError('Invalid thumbnail number');
        this.number.text = text; this.gutter.visible = Boolean(text); this.positionContent();
    }
    setTitle(text) {
        if (typeof text !== 'string' || text.length > 512) throw TypeError('Invalid thumbnail title');
        this.button.setProperty('tooltip', text || null); this.tooltip.text = text; this.tooltip.visible = Boolean(text);
    }
    setSelected(value) { return this.setProperty('selected', value); }
    setDropPosition(position){if(position!==null&&!['before','after'].includes(position))throw TypeError('Invalid thumbnail drop position');this.dropPosition=position;this.dropMarker.visible=position!==null;this.positionContent();}
    setLayout({ numberPlacement = this.numberPlacement, padding = this.padding || {}, aspectRatio = this.aspectRatio ?? null, contentWidth = this.contentWidth ?? null } = {}) {
        if (!['leading', 'below'].includes(numberPlacement) || !padding || typeof padding !== 'object' || Array.isArray(padding) ||
            Object.keys(padding).some(key => !['left','right','top','bottom'].includes(key)) ||
            Object.values(padding).some(value => !Number.isFinite(value) || value < 0 || value > 128) ||
            aspectRatio !== null && (!Number.isFinite(aspectRatio) || aspectRatio < .01 || aspectRatio > 1000) ||
            contentWidth !== null && (!Number.isFinite(contentWidth) || contentWidth < 1 || contentWidth > 100000)) throw TypeError('Invalid thumbnail layout');
        this.numberPlacement = numberPlacement; this.padding = { left:0, right:0, top:0, bottom:0, ...padding };
        this.aspectRatio = aspectRatio; this.contentWidth = contentWidth;
        this.number.setProperty('state', numberPlacement === 'below' ? State.START_ALIGNED : State.CENTER_ALIGNED);
        this.positionContent();
    }
    setSize(width, height) {
        if (![width, height].every(value => Number.isFinite(value) && value > 0 && value <= 100000)) throw RangeError('Invalid thumbnail size');
        this.layoutWidth = width; this.layoutHeight = height; this.positionContent();
    }
    get previewSize() { return this.layoutWidth ? { width: this.previewWidth, height: this.previewHeight } : null; }
    positionContent() {
        if (!this.layoutWidth) return;
        const below = this.numberPlacement === 'below', numbered = this.gutter.visible, pad = this.padding;
        const left = Math.min(this.layoutWidth - 1, pad.left + (!below && numbered ? this.numberWidth + 4 : 0)), top = Math.min(this.layoutHeight - 1, pad.top);
        let width = Math.max(1, this.layoutWidth-left-pad.right), height = Math.max(1, this.layoutHeight-top-pad.bottom-(below && numbered ? 24 : 0));
        if (this.contentWidth !== null) width = Math.min(width, this.contentWidth);
        if (this.aspectRatio !== null) { width = Math.min(width, height*this.aspectRatio); height = width/this.aspectRatio; }
        const rect = (x,y,w,h) => {
            const px = Math.max(0, Math.min(999, Math.round(x*1000/this.layoutWidth)));
            const py = Math.max(0, Math.min(999, Math.round(y*1000/this.layoutHeight)));
            return `${px}:${py}:${Math.max(1,Math.min(1000-px,Math.round(w*1000/this.layoutWidth)))}:${Math.max(1,Math.min(1000-py,Math.round(h*1000/this.layoutHeight)))}`;
        };
        if(this.dropMarker)this.dropMarker.text='positioned-item:'+(below
            ?rect(this.dropPosition==='after'?Math.max(0,this.layoutWidth-6):6,6,3,Math.max(1,this.layoutHeight-26))
            :rect(left,this.dropPosition==='after'?Math.max(0,this.layoutHeight-3):2,Math.max(1,this.layoutWidth-left),2));
        this.gutter.text = 'positioned-item:' + (below ? rect(left, top+height+1, width, Math.min(22,this.layoutHeight-top-height-1)) : rect(pad.left, top, this.numberWidth, Math.min(22,this.layoutHeight-top)));
        this.bodyFrame.text = 'positioned-item:' + rect(left,top,width,height);
        // One size observer per tile keeps long virtualized lists within the
        // native observation bound. Callers may observe body when needed.
        this.previewWidth = width; this.previewHeight = height;
        const x = Math.min(1000, Math.ceil(this.borderWidth * 1000 / width));
        const y = Math.min(1000, Math.ceil(this.borderWidth * 1000 / height));
        [[0, 0, 1000, y], [0, 1000-y, 1000, y], [0, 0, x, 1000], [1000-x, 0, x, 1000]]
            .forEach((rect, index) => { this.edges[index].text = `positioned-item:${rect.join(':')}`; });
    }
    updateSelection() { for (const edge of this.edges) edge.setProperty('state', this.getProperty('selected') ? State.PRESSED : State.FLAT); }
}

/** Caller-owned, bounded percentage scale for grids, previews and surfaces. */
export class ZoomModel {
    static controlFactory = Object.freeze({
        topics: ['visual', 'documents', 'product', 'navigation'],
        constructor: 'new ZoomModel({min:50,max:300,value:100,ready:true})',
        methods: 'view, setZoom(percent), setReady(boolean), onViewChanged(listener), dispose(); subscribes immediately with an immutable {zoom,percent,ready} view. Percentages round and clamp to the model bounds. Bind ZoomControl to this model and apply its view changes to caller-owned preview layout or page scale. No document edits or history are created. Fixed-percentage models have supportsFit:false; ZoomControl omits Fit automatically.',
    });
    constructor({min=10,max=400,value=100,ready=true}={}) {
        if(!Number.isSafeInteger(min)||!Number.isSafeInteger(max)||min<1||max>10000||min>=max||!Number.isFinite(value)||typeof ready!=='boolean')throw TypeError('Invalid zoom model');
        this.min=min;this.max=max;this.supportsFit=false;this.listeners=new Set();this.disposed=false;
        const percent=this.normalize(value);this.current=Object.freeze({zoom:percent,percent,ready});
    }
    normalize(value){if(typeof value!=='number'||!Number.isFinite(value))throw TypeError('Zoom requires a finite percentage');return Math.max(this.min,Math.min(this.max,Math.round(value)));}
    get view(){return this.current;}
    setZoom(value){
        const percent=this.normalize(value);
        if(this.disposed||!this.current.ready||percent===this.current.percent)return false;
        this.publish({zoom:percent,percent,ready:this.current.ready});return true;
    }
    setReady(ready){
        if(typeof ready!=='boolean')throw TypeError('Zoom readiness requires a boolean');
        if(this.disposed||ready===this.current.ready)return false;
        this.publish({...this.current,ready});return true;
    }
    publish(view){this.current=Object.freeze(view);for(const listener of [...this.listeners])listener(this.current);}
    onViewChanged(listener){
        if(typeof listener!=='function'||this.disposed)throw TypeError('Zoom subscription requires a live model and listener');
        this.listeners.add(listener);listener(this.current);return()=>this.listeners.delete(listener);
    }
    dispose(){if(this.disposed)return;this.setReady(false);this.disposed=true;this.listeners.clear();}
}

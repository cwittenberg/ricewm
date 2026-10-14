const MODES = new Set(["Center", "Tile", "Stretch", "Fit", "Crop"]);
const BACKGROUND_STYLES = new Set(["Solid", "Linear", "Radial", "Layered"]);

function checkedChannels(value) {
    if (
        !Array.isArray(value) ||
        value.length !== 4 ||
        value.some(channel =>
            !Number.isInteger(channel) || channel < 0 || channel > 255
        )
    ) {
        throw new TypeError("invalid desktop background color");
    }
    return value;
}

function canvasColor(value, alpha = checkedChannels(value)[3] / 255) {
    const channels = checkedChannels(value);
    return `rgba(${channels[0]}, ${channels[1]}, ${channels[2]}, ${alpha})`;
}

function gradientColor(background, name) {
    return checkedChannels(background.gradient?.[name] || background.color);
}

function gradientChannelEnabled(background, name) {
    return background.gradient?.enabled?.[name] !== false;
}

function enabledGradientColors(background, names) {
    return names
        .filter(name => gradientChannelEnabled(background, name))
        .map(name => gradientColor(background, name));
}

function addEvenColorStops(gradient, colors) {
    if (colors.length === 1) {
        gradient.addColorStop(0, canvasColor(colors[0]));
        gradient.addColorStop(1, canvasColor(colors[0]));
        return;
    }
    colors.forEach((color, index) => {
        gradient.addColorStop(index / (colors.length - 1), canvasColor(color));
    });
}

function fillBackground(context, background, frame) {
    context.fillStyle = canvasColor(background.color || [54, 79, 107, 255]);
    context.fillRect(frame.x, frame.y, frame.width, frame.height);

    const style = background.type === "Solid"
        ? "Solid"
        : BACKGROUND_STYLES.has(background.style)
            ? background.style
            : "Solid";
    if (style === "Linear" || style === "Layered") {
        const colors = enabledGradientColors(background, ["start", "middle", "end"]);
        if (colors.length > 0) {
            const diagonal = context.createLinearGradient(
                frame.x,
                frame.y,
                frame.x + frame.width,
                frame.y + frame.height,
            );
            addEvenColorStops(diagonal, colors);
            context.fillStyle = diagonal;
            context.fillRect(frame.x, frame.y, frame.width, frame.height);
        }
    }
    if (
        style === "Radial" ||
        (style === "Layered" && gradientChannelEnabled(background, "accent"))
    ) {
        const focusX = frame.x + frame.width * 0.5;
        const focusY = frame.y + frame.height * 0.42;
        const radius = Math.max(1, Math.hypot(frame.width, frame.height) * 0.42);
        const glow = context.createRadialGradient(
            focusX,
            focusY,
            0,
            focusX,
            focusY,
            radius,
        );
        const accent = gradientColor(background, "accent");
        if (style === "Layered") {
            glow.addColorStop(0, canvasColor(accent, 0.36));
            glow.addColorStop(0.58, canvasColor(accent, 0.1));
            glow.addColorStop(1, canvasColor(accent, 0));
        } else {
            const colors = enabledGradientColors(background, ["accent", "middle", "end"]);
            if (colors.length === 0) return;
            addEvenColorStops(glow, colors);
        }
        context.fillStyle = glow;
        context.fillRect(frame.x, frame.y, frame.width, frame.height);
    }
}

function imageSize(image) {
    const width = Number(image.displayWidth || image.naturalWidth || image.videoWidth || image.width);
    const height = Number(image.displayHeight || image.naturalHeight || image.videoHeight || image.height);
    return Number.isFinite(width) && width > 0 &&
        Number.isFinite(height) && height > 0
        ? { width, height }
        : null;
}

function placement(mode, frame, image) {
    if (mode === "Stretch") return { ...frame };
    const scale = mode === "Crop"
        ? Math.max(frame.width / image.width, frame.height / image.height)
        : Math.min(frame.width / image.width, frame.height / image.height);
    const width = Math.max(1, Math.floor(image.width * scale));
    const height = Math.max(1, Math.floor(image.height * scale));
    return {
        x: frame.x + Math.floor((frame.width - width) / 2),
        y: frame.y + Math.floor((frame.height - height) / 2),
        width,
        height,
    };
}

export class RiceNativeWallpaper {
    constructor(context, onDamage=()=>{}, now=()=>performance.now()) {
        this.context = context;this.onDamage=onDamage;this.now=now;this.flight=null;this.lastView={x:.5,y:.5,zoom:1};
    }

    render(background, frame) {
        if (!background) return;
        const scene=background.scene;
        const desired=scene?.crop||{x:.5,y:.5,zoom:1};
        const key=JSON.stringify([scene?.source||'',desired]);
        const now=this.now();
        if(this.flight&&this.viewKey===key){const t=Math.min(1,(now-this.flight.started)/Math.max(1,this.flight.duration));const eased=t*t*(3-2*t);this.lastView={x:this.flight.from.x+(this.flight.to.x-this.flight.from.x)*eased,y:this.flight.from.y+(this.flight.to.y-this.flight.from.y)*eased,zoom:Math.exp(Math.log(this.flight.from.zoom)+(Math.log(this.flight.to.zoom)-Math.log(this.flight.from.zoom))*eased)};if(t>=1)this.flight=null;}
        if(this.viewKey!==key){
            const duration=globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches?0:(scene?.duration||0);
            this.flight=this.viewKey&&duration?{from:{...this.lastView},to:desired,started:now,duration}:null;
            if(!this.flight)this.lastView={...desired};this.viewKey=key;
        }
        if(!scene?.duration||globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches){this.flight=null;this.lastView={...desired};}
        if(this.flight)this.onDamage();
        const selectedMode=scene?.mode||background.mode;
        const mode = MODES.has(selectedMode) ? selectedMode : "Crop";
        const context = this.context;
        fillBackground(context, background, frame);
        const image = background.type === "Image" ||
            background.type === "Video" || background.type == null
            ? background.surface
            : null;
        const size = image && imageSize(image);
        if (!size) return;
        context.save();
        context.beginPath();
        context.rect(frame.x, frame.y, frame.width, frame.height);
        context.clip();
        if(scene && scene.flying!==false){
            const view=this.lastView;
            const scale=Math.max(frame.width/size.width,frame.height/size.height)*view.zoom;
            const sw=frame.width/scale,sh=frame.height/scale;
            const sx=Math.min(size.width-sw,Math.max(0,view.x*size.width-sw/2));
            const sy=Math.min(size.height-sh,Math.max(0,view.y*size.height-sh/2));
            context.imageSmoothingEnabled=true;context.imageSmoothingQuality='high';
            context.drawImage(image,sx,sy,sw,sh,frame.x,frame.y,frame.width,frame.height);
        } else if (mode === "Tile") {
            const pattern = context.createPattern(image, "repeat");
            if (pattern) {
                context.fillStyle = pattern;
                context.fillRect(frame.x, frame.y, frame.width, frame.height);
            }
        } else if (mode === "Center") {
            context.drawImage(
                image,
                frame.x + Math.floor((frame.width - size.width) / 2),
                frame.y + Math.floor((frame.height - size.height) / 2),
            );
        } else {
            const target = placement(mode, frame, size);
            context.drawImage(
                image,
                target.x,
                target.y,
                target.width,
                target.height,
            );
        }
        context.restore();
    }

}

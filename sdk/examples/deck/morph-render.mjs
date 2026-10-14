import { SceneBuilder, remapSceneFrames, composeSceneFrame } from '@ricewm/app';
import { geometryFrame } from './scene-preview.mjs';
import { matchMorphObjects, multiplyAffine, inverseAffine, objectAffine, interpolateAffine } from './morph-matching.mjs';

/** Object-mode Morph. Decode/layout once, retain native command ranges and
 * interpolate their placement each tick. No screenshot or DOM surrogate. */
export function prepareMorph(previous, next) {
    const project = rendered => geometryFrame(rendered.scene, { textFrames: rendered.textFrames,
        imageFrames: rendered.imageFrames, objectOpacity: rendered.objectOpacity, captureLayers: true });
    const right = project(next), left = previous ? project(previous) : null;
    const remapped = remapSceneFrames(left ? [left.frame, right.frame] : [right.frame]);
    const before = left ? remapped[0] : null, after = remapped.at(-1);
    const {width,height} = after;
    const pop = new SceneBuilder(width,height).popTransform().commands[0];
    const clipEnd = new SceneBuilder(width,height).popClip().commands[0];
    const push = (matrix, opacity=1) => new SceneBuilder(width,height).pushTransform(matrix,opacity).commands[0];
    const scale = rendered => [width/rendered.scene.size.width,0,0,height/rendered.scene.size.height,0,0];
    function parts(projected, frame, rendered) {
        const {resourceStart,resourceEnd,backgroundEnd,objects} = projected.layers;
        const data = frame.displayList;
        return { resources:data.subarray(resourceStart,resourceEnd),
            background:[data.subarray(0,resourceStart),data.subarray(resourceEnd,backgroundEnd),pop],
            objects:new Map(objects.map(o => [o.id,data.subarray(o.start,o.end)])),
            content:new Map(objects.map(o => [o.id,data.subarray(o.contentStart,o.contentEnd)])),
            scene:rendered.scene, opacity:rendered.objectOpacity, byId:new Map(rendered.scene.objects.map(o=>[o.id,o])), scale:scale(rendered), frame };
    }
    const old = left ? parts(left,before,previous) : null, current=parts(right,after,next);
    const visible = owner => owner?.scene.objects.filter(o=>owner.objects.has(o.id)) || [];
    const {matches,unmatched}=matchMorphObjects(visible(old),visible(current));
    function appearance(object, owner) {
        const value = (object, root = false) => [object.geometry, object.paths, object.fill, object.stroke, object.image, object.outerShadow, root ? null : owner.opacity?.get(object.id) ?? 1,
            object.text && [object.text.text, object.text.style, object.text.layout, object.text.runs.map(run=>[run.text,run.style])], object.table];
        const byId = owner.byId;
        const inverse = object.transform && inverseAffine(object.transform);
        const descendants = [];
        function visit(parent) {
            for (const id of parent.group?.children || []) {
                const child=byId.get(id);
                if (!child) continue;
                descendants.push([value(child),child.bounds,inverse && child.transform && multiplyAffine(inverse,child.transform)]);
                visit(child);
            }
        }
        if (object.kind==='group') visit(object);
        return JSON.stringify([value(object,true),descendants]);
    }
    const colorOnly = (a,b) => a && b && a.kind!=='group' && b.kind!=='group' && a.fill?.opacity===1 && b.fill?.opacity===1 &&
        appearance({...a,fill:b.fill,stroke:b.stroke},old)===appearance(b,current) &&
        ((!a.stroke&&!b.stroke)||(a.stroke?.opacity===1&&b.stroke?.opacity===1&&a.stroke.width===b.stroke.width));
    const plans=visible(current).map(object=>{
        const prior=matches.get(object.id), a=prior&&objectAffine(prior), b=objectAffine(object);
        const inverseA=a&&inverseAffine(a), inverseB=b&&inverseAffine(b);
        return {object,prior,same:prior&&appearance(prior,old)===appearance(object,current),colorOnly:colorOnly(prior,object),a:a&&multiplyAffine(old.scale,a),b:b&&multiplyAffine(current.scale,b),inverseA,inverseB};
    });
    return progress=>{
        if (!Number.isFinite(progress)) throw new RangeError('Invalid Morph progress');
        const t=Math.max(0,Math.min(1,progress));
        if (t===0&&before) return before;
        if (t===1) return after;
        const chunks=[new SceneBuilder(width,height).pushClip([0,0,width,height]).commands[0]];
        if (old) chunks.push(old.resources);
        chunks.push(current.resources);
        const background=(owner,opacity)=>{
            if (!owner) return;
            chunks.push(push([width/owner.frame.width,0,0,height/owner.frame.height,0,0],opacity),...owner.background,pop);
        };
        background(old,1);background(current,old?t:1);
        const paint=(owner,object,matrix,opacity,contentOnly=false)=>{
            chunks.push(push(matrix,opacity),(contentOnly ? owner.content : owner.objects).get(object.id),pop);
        };
        for(const object of unmatched) paint(old,object,old.scale,1-t);
        for(const plan of plans){
            const {object,prior,a,b,inverseA,inverseB}=plan;
            if(prior&&inverseA&&inverseB){
                const matrix=interpolateAffine(a,b,t);
                // Own animation opacity is an independent channel. Interpolate
                // it once around the retained content blend, so equal partial
                // opacity stays constant and differing opacity changes linearly.
                const opacityA = old.opacity?.get(prior.id) ?? 1, opacityB = current.opacity?.get(object.id) ?? 1;
                const opacity = opacityA+(opacityB-opacityA)*t;
                // Hidden endpoint artwork contributes no color to the blend.
                // Geometry still follows t; visible content uses normalized
                // premultiplied-alpha weights inside the one opacity scope.
                const mix = opacity > 0 ? t*opacityB/opacity : t;
                chunks.push(push([1,0,0,1,0,0],opacity));
                if (!plan.same) paint(old,prior,multiplyAffine(matrix,inverseA),plan.colorOnly?1:1-mix,true);
                paint(current,object,multiplyAffine(matrix,inverseB),plan.same?1:mix,true);
                chunks.push(pop);
            }else{
                if(prior) paint(old,prior,old.scale,1-t);
                paint(current,object,current.scale,t);
            }
        }
        chunks.push(clipEnd);
        return composeSceneFrame(width,height,chunks);
    };
}

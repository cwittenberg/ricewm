import { exact, failure } from './contract-values.mjs';

export const IMAGE_BYTES = 4 * 1024 * 1024;
export const IMAGE_PIXELS = 4 * 1024 * 1024;
export const IMAGE_COMMANDS = new Set(['insert-image', 'replace-image', 'set-image-crop']);
export const imageAsset = value => typeof value === 'string' && /^image:sha256:[0-9a-f]{64}$/.test(value);
export const imageCrop = value => exact(value, ['left', 'top', 'right', 'bottom']) &&
    Object.values(value).every(v => Number.isFinite(v) && v >= 0 && v < 1) &&
    value.left + value.right < 1 && value.top + value.bottom < 1;
const dimensions = value => ['width', 'height'].every(key => Number.isInteger(value[key]) && value[key] > 0 && value[key] <= 8192) && value.width * value.height <= IMAGE_PIXELS;
const length = value => Number.isSafeInteger(value) && value > 0 && value <= IMAGE_BYTES;
const mime = value => ['image/png', 'image/jpeg', 'image/svg+xml'].includes(value);
export function imageMetadata(value) {
    return exact(value, ['asset', 'mime', 'width', 'height', 'length']) && imageAsset(value.asset) && mime(value.mime) && dimensions(value) && length(value.length);
}
export function sceneImage(value) {
    return value === null || exact(value, ['asset', 'mime', 'width', 'height', 'byteLength', 'crop', 'canCrop', 'canReplace']) &&
        imageAsset(value.asset) && mime(value.mime) && dimensions(value) && length(value.byteLength) && imageCrop(value.crop) &&
        typeof value.canCrop === 'boolean' && typeof value.canReplace === 'boolean';
}
export function checkedImageReply(reply, expected, asset) {
    if (reply?.revision !== expected || !imageMetadata(reply?.result) || (asset !== undefined && reply.result.asset !== asset)) {
        throw failure('INVALID_RESPONSE', 'Invalid picture snapshot');
    }
    return reply;
}
export function checkedImageCommand(command) {
    const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
    let valid = id(command?.slide);
    if (command?.type === 'insert-image') {
        const box = command.box;
        valid &&= exact(command, ['type', 'slide', 'asset', 'box']) && imageAsset(command.asset) &&
            exact(box, ['x', 'y', 'width', 'height']) && Object.values(box).every(v => Number.isFinite(v) && Math.abs(v) <= 1e6) && box.width > 0 && box.height > 0;
    } else if (command?.type === 'replace-image') {
        valid &&= exact(command, ['type', 'slide', 'object', 'asset']) && id(command.object) && imageAsset(command.asset);
    } else if (command?.type === 'set-image-crop') {
        valid &&= exact(command, ['type', 'slide', 'object', 'crop']) && id(command.object) && imageCrop(command.crop);
    } else valid = false;
    if (!valid) throw failure('INVALID_REQUEST', 'Invalid picture command');
    return structuredClone(command);
}

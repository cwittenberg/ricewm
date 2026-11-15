import {TABLE_RANGE_COMMANDS,checkedTableRangeCommand} from './table-range-contract.mjs';
import {TABLE_MERGE_COMMANDS,checkedTableMergeCommand} from './table-merge-contract.mjs';
import {TABLE_DESIGN_COMMANDS,checkedTableDesignCommand} from './table-design-contract.mjs';
import { checkedHyperlinkCommand } from './hyperlink-contract.mjs';
import { TABLE_COMMANDS, checkedTableCommand, tableArguments } from './table-contract.mjs';
import { selectionArguments } from './selection-contract.mjs';
import {slideClipboardArguments,slideClipboardToken,checkedPasteSlides} from './slide-transfer-contract.mjs';
import { notesArguments, checkedNotesCommand } from './notes-contract.mjs';
import { exact, failure, object } from './contract-values.mjs';
export { exact, failure, object } from './contract-values.mjs';
/** Semantic data shared by the Deck adapter and native UI. No document model. */
import { IMAGE_COMMANDS, checkedImageCommand, imageAsset } from './image-contract.mjs';
import { checkedObjectCommand } from './authoring-contract.mjs';
import {TEXT_RANGE_COMMANDS,checkedTextRangeCommand} from './text-range-contract.mjs';
import { BACKGROUND_COMMANDS, checkedBackgroundCommand } from './background-contract.mjs';
import {THEME_COMMANDS,checkedThemeCommand} from './theme-contract.mjs';
import { checkedTransitionCommand } from './transition-contract.mjs';
import { checkedShadowCommand } from './shadow-contract.mjs';
import { animationArguments } from './animation-contract.mjs';
import { ANIMATION_COMMANDS, checkedAnimationCommand, checkedCreatedAnimations } from './animation-authoring-contract.mjs';
import { SLIDE_COMMANDS, checkedSlideCommand, outlineArguments, outlineResult } from './slide-contract.mjs';
import { SECTION_COMMANDS, checkedSectionCommand, sectionArguments, sectionNameArguments } from './section-contract.mjs';
export const BUFFER_BYTES = 65536;
export const FILE_BYTES = 8 * 1024 * 1024;
import { checkedStatus, integer, text, revision } from './status-contract.mjs';
export { FORMATS, normalizeError, rejectedRevision, revision, checkedResponse, checkedStatus } from './status-contract.mjs';
export function encode(value) {
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    if (bytes.length > BUFFER_BYTES) throw failure('PAYLOAD_LIMIT', 'Deck command exceeds 64 KiB');
    return bytes;
}
export function checkedRequest(operation, expectedRevision, args = {}) {
    let arguments_;
    if (operation === 'project') {
        if (!exact(args, ['slide', 'startRun', 'limit'], [])) throw failure('INVALID_REQUEST', 'Invalid slide projection');
        arguments_ = { slide: args.slide ?? 0, startRun: args.startRun ?? 0, limit: args.limit ?? 32 };
        if (!integer(arguments_.slide) || !integer(arguments_.startRun) || !integer(arguments_.limit, 64) || arguments_.limit < 1) {
            throw failure('INVALID_REQUEST', 'Invalid bounded slide projection');
        }
    } else if (operation === 'scene') {
        if (!exact(args, ['slide', 'startObject', 'limit'], [])) throw failure('INVALID_REQUEST', 'Invalid scene projection');
        arguments_ = { slide: args.slide ?? 0, startObject: args.startObject ?? 0, limit: args.limit ?? 32 };
        if (!integer(arguments_.slide) || !integer(arguments_.startObject) || !integer(arguments_.limit, 64) || arguments_.limit < 1) {
            throw failure('INVALID_REQUEST', 'Invalid bounded scene projection');
        }
    } else if(['prepare-theme','discard-theme'].includes(operation)){
        if(!exact(args,[]))throw failure('INVALID_REQUEST','Invalid theme preparation request');arguments_={};
    } else if(operation==='section-outline') {
        arguments_=sectionArguments(args);
    } else if(operation==='section-name') {
        arguments_=sectionNameArguments(args);
    } else if (operation === 'animation-timeline') {
        arguments_ = animationArguments(args);
    } else if (operation === 'table-cells') {
        arguments_ = tableArguments(args);
    } else if (operation === 'speaker-notes') {
        arguments_ = notesArguments(args);
    } else if (operation === 'outline') {
        arguments_ = outlineArguments(args);
    } else if (operation === 'apply') {
        if (!exact(args, ['commands']) || !Array.isArray(args.commands) || !args.commands.length || args.commands.length > 32) {
            throw failure('INVALID_REQUEST', 'Deck requires a batch of 1 to 32 commands');
        }
        arguments_ = { commands: args.commands.map(command => {
            if(command?.type==='paste-slides')return checkedPasteSlides(command);
            if(SECTION_COMMANDS.has(command?.type))return checkedSectionCommand(command);
            if (command?.type === 'change-sensitivity') { if (!exact(command,['type','change'])) throw failure('INVALID_REQUEST','Invalid sensitivity command'); if (!exact(command.change,['currentId','siteId','id']) || !Object.values(command.change).every(v=>typeof v==='string'&&v.length<=38)) throw failure('INVALID_REQUEST','Invalid embedded label selection'); return command; }
            if (command?.type === 'set-hyperlink') return checkedHyperlinkCommand(command);
            if (ANIMATION_COMMANDS.has(command?.type)) return checkedAnimationCommand(command);
            if (command?.type === 'set-object-shadow') return checkedShadowCommand(command);
            if (['set-slide-transition', 'set-selected-slide-transitions'].includes(command?.type)) return checkedTransitionCommand(command);
            if (BACKGROUND_COMMANDS.has(command?.type)) return checkedBackgroundCommand(command);
            if (THEME_COMMANDS.has(command?.type)) return checkedThemeCommand(command);
            if(TABLE_RANGE_COMMANDS.has(command?.type))return checkedTableRangeCommand(command);
            if (TABLE_MERGE_COMMANDS.has(command?.type)) return checkedTableMergeCommand(command);
            if (TABLE_DESIGN_COMMANDS.has(command?.type)) return checkedTableDesignCommand(command);
            if (TABLE_COMMANDS.has(command?.type)) return checkedTableCommand(command);
            if (command?.type === 'set-speaker-notes') return checkedNotesCommand(command);
            if (IMAGE_COMMANDS.has(command?.type)) return checkedImageCommand(command);
            if (SLIDE_COMMANDS.has(command?.type)) return checkedSlideCommand(command);
            if (TEXT_RANGE_COMMANDS.has(command?.type)) return checkedTextRangeCommand(command);
            if (command?.type !== 'set-text') return checkedObjectCommand(command);
            if (!exact(command, ['type', 'slide', 'run', 'text']) || command.type !== 'set-text' ||
                !text(command.slide, 4096) || !command.slide || !text(command.run, 4096) || !command.run || !text(command.text, 32768)) {
                throw failure('INVALID_REQUEST', 'Invalid ordinary text command');
            }
            return { ...command };
        }) };
    } else if (operation === 'export-selection') {
        arguments_ = selectionArguments(args);
    } else if(operation==='export-slides') {
        arguments_=slideClipboardArguments(args);
    } else if(operation==='confirm-slide-copy') {
        if(!exact(args,['selection'])||!slideClipboardToken(args.selection))throw failure('INVALID_REQUEST','Invalid copied slide snapshot');
        arguments_={selection:args.selection};
    } else if (operation === 'image') {
        if (!exact(args, ['asset']) || !imageAsset(args.asset)) throw failure('INVALID_REQUEST', 'Invalid picture identity');
        arguments_ = { asset: args.asset };
    } else if (['shape-catalog', 'layout-catalog', 'animation-catalog', 'status', 'undo', 'redo', 'begin-save', 'mark-saved', 'prepare-image', 'discard-image', 'prepare-selection', 'discard-selection','prepare-slides','discard-slides'].includes(operation)) {
        if (!exact(args, [])) throw failure('INVALID_REQUEST', 'Unexpected Deck operation arguments');
        arguments_ = {};
    } else throw failure('UNSUPPORTED_OPERATION', 'Unsupported Deck operation');
    const request = { operation, arguments: arguments_ };
    if (expectedRevision !== undefined) request.expectedRevision = revision(expectedRevision);
    else if (!['shape-catalog', 'layout-catalog', 'status', 'project'].includes(operation)) throw failure('INVALID_REVISION', 'Mutations require an expected revision');
    encode(request);
    return request;
}
export function checkedProjection(value) {
    checkedStatus(value);
    const data = value.result;
    if (!integer(data.slideCount, 4096) || !['dirty', 'canUndo', 'canRedo'].every(key => typeof data[key] === 'boolean')) {
        throw failure('INVALID_RESPONSE', 'Invalid Deck projection status');
    }
    const slide = data.slide;
    if (slide === null) {
        if (data.slideCount !== 0 || data.nextRun !== null) throw failure('INVALID_RESPONSE', 'Invalid empty presentation');
    } else {
        if (!object(slide) || !integer(slide.index) || slide.index >= data.slideCount || !text(slide.id, 4096) || !slide.id ||
            !text(slide.name, 65536) || !text(slide.part, 4096) || !integer(slide.runCount, 100000) ||
            !Array.isArray(slide.text) || slide.text.length > 64 ||
            (data.nextRun !== null && (!integer(data.nextRun) || data.nextRun >= slide.runCount))) {
            throw failure('INVALID_RESPONSE', 'Invalid Deck slide projection');
        }
        const ids = new Set();
        let previous = -1;
        for (const run of slide.text) {
            if (!object(run) || !integer(run.index) || run.index <= previous || run.index >= slide.runCount ||
                !text(run.id, 4096) || !run.id || ids.has(run.id) || !text(run.text, 65536) || typeof run.editable !== 'boolean') {
                throw failure('INVALID_RESPONSE', 'Invalid Deck text projection');
            }
            ids.add(run.id); previous = run.index;
        }
        if (data.nextRun !== null && data.nextRun <= previous) throw failure('INVALID_RESPONSE', 'Invalid Deck projection continuation');
    }
    return value;
}

export function checkedOutline(value, args) {
    checkedStatus(value);
    outlineResult(value.result, args);
    return value;
}

export function checkedMutation(value, commands) {
    checkedStatus(value);
    checkedCreatedAnimations(value.result, commands);
    const count = commands.reduce((total,command)=>total+(command.type==='paste-slides'?command.count:command.type==='duplicate-selected-slides'?command.slides.length:Number(['insert-slide','duplicate-slide'].includes(command.type))),0);
    const created = value.result.createdSlides;
    if (created === undefined && !count) return value;
    if (!Array.isArray(created) || created.length !== count || new Set(created).size !== created.length ||
        created.some(id => !text(id, 4096) || !id)) throw failure('INVALID_RESPONSE', 'Invalid created slide identities');
    return value;
}

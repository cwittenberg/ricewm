import { defineApp, createCancellationScope } from '@ricewm/app';
import { startDeckShell } from './shell.mjs';
import { createDeckImageLayout } from './image-layout.mjs';
import { createDeckTextLayout } from './text-layout.mjs';
import { createDeckDocumentPort } from './document-port.mjs';

export default defineApp({
    manifest: { id: 'org.ricewm.deck', name: 'Rice Deck', version: '0.11.2-preview.1' },
    start(context) {
        // Window construction must finish before a host-owned chooser is opened.
        return startDeckShell(context, createDeckDocumentPort({ ...context, createTextLayout: createDeckTextLayout, createImageLayout: createDeckImageLayout, lifetime: createCancellationScope() }));
    },
});

import { defineApp } from "@ricewm/app";
import { startSheetShell } from "./shell.mjs";
import { openSheetDocumentPort } from "./document-port.mjs";

export default defineApp({
    manifest: { id: "org.ricewm.sheet", name: "Rice Sheet", version: "0.1.0" },
    async start(context) {
        // Every window owns its service, session and compute worker. RiceBus is
        // local when the provider and caller share this application worker.
        try {
            return startSheetShell(context, await openSheetDocumentPort(context));
        } catch (error) {
            context.log?.warn('Sheet workbook startup failed', { message: String(error.message) });
            return startSheetShell(context);
        }
    },
});

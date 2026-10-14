import { createAppAboutDialog } from "@ricewm/app";

/** Product information and credits for components actually shipped. Full license
 * texts and copyright notices remain bundled in the application's licenses. */
export function createAboutDialog(onClose) {
    return createAppAboutDialog({
        name: "Rice Sheet", icon: 109,
        tagline: "Spreadsheets, made clear. · Rice Office · 0.1.0",
        sections: [
            ["About", "# Rice Sheet\nRice Office · 0.1.0\n\nCreate, calculate and explore your spreadsheets."],
            ["Credits", "# Credits\nRiceWM and Rice Office contributors.\n\nSpreadsheet calculation: IronCalc.\nText rendering: HarfRust and Skrifa.\nFonts: Liberation."],
        ],
        onClose,
    });
}

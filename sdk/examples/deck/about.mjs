import { createAppAboutDialog } from '@ricewm/app';

export function createAboutDialog(onClose) {
    return createAppAboutDialog({ name: 'Rice Deck', icon: 104,
        tagline: 'Native presentations · Rice Office', onClose,
        sections: [
            ['Overview', '## Present with Rice Deck\n\nRice Deck is a lightweight, native RiceWM presentation app in the Rice Office suite, alongside Rice Paper and Rice Sheet. Create, edit and present slides with shared desktop themes, controls and file integration.'],
            ['What it can do', '## What Rice Deck can do\n\nCreate slides, edit text boxes, draw shapes, insert pictures and adjust colors and alignment. Browse slide thumbnails and present from the beginning or the current slide. Undo and redo edits, then save your presentation.'],
            ['Standards status', '## Presentation formats\n\nOpen and save supported PowerPoint (.pptx) and OpenDocument (.odp) presentations. PowerPoint templates and slide-show files have dedicated open and save actions.\n\nSupport is still developing; complex documents may not preserve every feature.'],
            ['Limitations', '## Compatibility and limits\n\nCharts, inherited layouts, animations and media remain in development. Full PowerPoint and OpenDocument fidelity is not yet qualified. Keep an original copy when editing presentations from other applications.'],
            ['Credits & licenses', '## Credits & licenses\n\nRiceWM, RiceUI and the RiceApp SDK provide the application and native controls. The native Wasm Rice Deck engine handles presentation documents; the shared RiceApp and RiceBus capabilities connect it to the desktop.\n\nThe application package includes the RiceWM license, dependency notices in the licenses directory, and a components.json inventory of the shipped engine dependencies.'],
        ],
    });
}

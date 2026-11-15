import {SDK_RESOURCE_PROFILES} from '@ricewm/app';

// Keep existing typed aliases first; actual reference families come from the
// selected SDK's reflected font inventory, shared by both native pickers.
export const deckFontFamilies=Object.freeze([...new Set([
    'Arial','Liberation Sans','Manrope','Noto Sans Arabic','Noto Sans JP',
    ...SDK_RESOURCE_PROFILES.profiles.typography.fonts.map(font=>font.family),
])]);

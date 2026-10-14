/** Shell appearance is separate from an upstream widget's configuration keys. */
export function widgetSurfacePreferences(value = {}) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !['background','border'].includes(key))) throw new TypeError('Invalid widget surface preferences');
    const background=value.background ?? 'auto', border=value.border ?? true;
    if (!['auto','none','standard','translucent','shadow'].includes(background) || typeof border !== 'boolean') throw new TypeError('Invalid widget surface appearance');
    return {background,border};
}

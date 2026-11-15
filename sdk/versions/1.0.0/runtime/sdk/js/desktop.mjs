/** Subscribe through app.bus using these exact manifest subscribe grants.
 * The host delivers these events directly to the approved worker. */
export function desktopEventTopics(appId) {
    if(typeof appId !== 'string' || !/^[a-z0-9][a-z0-9.-]+$/.test(appId)) throw TypeError('Application identity is required');
    return Object.freeze({
        mediaChanged:`${appId}/media/changed`,
        jobsChanged:`${appId}/jobs/changed`,
        notificationAction:`${appId}/notifications/action`,
        calendarChanged:`${appId}/calendar/changed`,
        wallpaperChanged:`${appId}/wallpapers/changed`,
        hotkeyActivated:`${appId}/hotkeys/activated`,
        actionInvoked:`${appId}/actions/invoke`,
        pluginInvoked:`${appId}/plugins/invoke`,
        fileOpenChanged:`${appId}/files/open-status`,
        fileChanged:`${appId}/files/changed`,
        capabilityInvoked:`${appId}/capabilities/invoke`,
        dropEvent:`${appId}/drops/event`,
        terminalEvent:`${appId}/terminal/event`,
    });
}

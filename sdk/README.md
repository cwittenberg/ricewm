# RiceWM application SDK 1.0.0

Apps import `@ricewm/app`, declare an exact SDK version and request only the
host services they need. The SDK provides controls, layouts, messages, scenes,
charts and capability-checked host calls. Compatible changes update 1.0.0.

The implementation is at `versions/1.0.0/runtime/sdk/js/`. Import
`riceSdkLibrary` from `sdk/rice-sdk-versions.mjs` when inspecting the supported
release. Service metadata is reflected from this implementation.

## Package an example

From the repository root, with Node.js 18 or newer:

```sh
node tools/riceapp-pack.mjs sdk/examples/calc-sdk /tmp/calc-sdk.riceapp
node tools/riceapp-pack.mjs sdk/examples/todo /tmp/todo.riceapp
```

Copy an example, change its manifest ID and name, and keep
`"sdk": { "version": "1.0.0" }` in `manifest.json`. Its entry module exports a
`defineApp` application. The packager validates the SDK selection and service
permissions using the actual SDK, then writes a ZIP-format `.riceapp` archive.
It does not change your source. Install the archive in a compatible RiceWM host.
Host-side installation, qualification and capability enforcement remain required.

## Examples

| Directory | Demonstrates |
| --- | --- |
| `calc-sdk` | Buttons, menus, dialogs and calculator state |
| `sdk-guide` | Interactive controls and starter-project creation |
| `date-time` | Date and time controls |
| `todo` | Lists, task details, steps and local settings |
| `chart-dashboard` | Interactive charts, themes and native controls |
| `table-report` | Tables and report presentation |
| `notification-reminders` | Persistent reminders through the notifications service |
| `desktop-capabilities` | Notifications, calendar, hotkeys and desktop actions |
| `performance-sdk` | Host telemetry and system-monitor controls |
| `visual-recipes` | Theme and desktop recipe services |
| `sheet` / `deck` | Spreadsheet and presentation application sources |

Examples execute inside a RiceWM app host, not directly as browser pages or
Node.js programs. Services depend on the host and declared grants. `npm test`
uses local bridges to validate capability calls without a live desktop.
Sheet and Deck additionally require compiled engines, fonts and other assets
that are not distributed here; their source archives alone are not runnable apps.

The SDK includes Apache ECharts with its Apache-2.0 and dependency notices in
`versions/1.0.0/runtime/sdk/js/vendor/echarts/`.

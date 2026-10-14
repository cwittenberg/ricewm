import {
    Button,
    Column,
    Container,
    Label,
    Markdown,
    Message,
    Row,
    ScrollView,
    SplitPane,
    State,
    defineApp,
} from "@ricewm/app";

const enabled = State.ENABLED;
const projectDirectory = "starter";

const starterManifest = `{
  "schema": "ricewm.app/v1",
  "id": "dev.example.hello-ricewm",
  "name": "Hello RiceWM",
  "version": "1.0.0",
  "description": "My first JavaScript RiceApp.",
  "entry": "app.mjs",
  "categories": ["Development"],
  "permissions": { "services": [], "bus": {} }
}
`;

// Split the module keyword so the guide's own dependency policy does not
// mistake generated source for another dependency of this package.
const starterSource = "im" + `port {
    Button, Column, Label, Message, State, defineApp,
} fr` + `om "` + "@" + `ricewm/app";
` + "im" + `port { counterMessage } from "./counter-message.mjs";

// RiceUI controls are retained objects. Update a property and the SDK sends
// only the bounded patch that RiceWM needs to redraw.
export default defineApp({
    manifest: {
        id: "dev.example.hello-ricewm",
        name: "Hello RiceWM",
        version: "1.0.0",
    },
    start({ createWindow, console }) {
        let count = 0;
        const output = new Label({
            text: "Ready — select the button",
            state: State.ENABLED | State.INFORMATION,
        });
        const button = new Button({ text: "Count: 0" });
        button.on(Message.ACTIVATE, () => {
            count += 1;
            button.text = \`Count: \${count}\`;
            output.text = counterMessage(count);
            console.log("counter changed", { count });
        });
        createWindow({
            title: "Hello RiceWM",
            root: new Column({}, [
                new Label({ text: "Hello from JavaScript!", state: State.DISPLAY }),
                output,
                button,
            ]),
        });
    },
});
`;

const starterCounterModule = `// Package-local modules use explicit relative .mjs imports.
export function counterMessage(count) {
    return \`The retained event handler ran \${count} time(s).\`;
}
`;

const APP_NAME_INDICATOR = "appnameindicator@dev64.xyz";
// Keep ordinary GJS imports as display text without making them dependencies
// of this RiceApp package.
const extensionBridgeExample = "im" + "port Gio fr" + `om 'gi://Gio';

const Proxy = Gio.DBusProxy.makeProxyWrapper(XML);
const apps = new Proxy(Gio.DBus.session, 'org.ricewm.Apps', '/org/ricewm/Apps');
const [json] = await apps.CallRemote(
    'org.ricewm.extension/demo',
    JSON.stringify({ greeting: 'Hello from GNOME' }),
);

const exported = Gio.DBusExportedObject.wrapJSObject(MY_XML, {
    Echo: text => [\`Extension received: \${text}\`],
});
exported.export(Gio.DBus.session, '/org/example/MyExtension');`;

export default defineApp({
    manifest: {
        id: "dev.ricewm.sdk-guide",
        name: "RiceApp SDK Guide",
        version: "1.2.0",
    },
    async start({ bus, console, createWindow, services }) {
        let activeSection = "Start here";
        let status = "Preparing your editable starter project…";
        let reloads = 0;
        let demoRevision = 0;
        let statusLabel;
        let reloadLabel;
        let readingLabel;
        let extensionStatusLabel;
        let extensionToggleButton;
        let extensionRecords = [];
        let extensionStatus = "The protected extension catalog has not been read yet.";
        let documentScroll;
        let window;
        const navigationButtons = new Map();
        const sectionAnchors = new Map();

        const updateLiveStatus = () => {
            if (statusLabel) statusLabel.text = status;
            if (reloadLabel) {
                reloadLabel.text = `Successful automatic reloads: ${reloads}`;
            }
        };

        const activate = (control, callback) => {
            control.on(Message.ACTIVATE, callback);
            return control;
        };
        const copy = text => void services.clipboard.writeText(text).then(() => {
            status = "Copied to the clipboard.";
            updateLiveStatus();
        }).catch(error => fail("Copy failed", error));
        const fail = (prefix, error) => {
            status = `${prefix}: ${String(error?.message || error)}`;
            console.error(status);
            updateLiveStatus();
        };
        const action = (text, callback, icon = null, primary = false) => activate(new Button({
            text,
            icon,
            state: enabled | (primary ? State.ACCENT : 0),
        }), callback);
        const updateExtensionStatus = () => {
            if (extensionStatusLabel) extensionStatusLabel.text = extensionStatus;
            if (extensionToggleButton) {
                const indicator = extensionRecords.find(record =>
                    record.uuid === APP_NAME_INDICATOR
                );
                extensionToggleButton.text = indicator?.enabled
                    ? "Disable App Name Indicator"
                    : "Enable App Name Indicator";
            }
        };
        const refreshExtensions = async () => {
            try {
                extensionRecords = await services.extensions.list();
                extensionStatus = extensionRecords.length
                    ? `${extensionRecords.length} installed extension(s).`
                    : "No extensions are installed.";
            } catch (error) {
                extensionStatus = `Extension catalog failed: ${String(error?.message || error)}`;
                console.error(extensionStatus);
            }
            updateExtensionStatus();
        };
        const toggleAppNameIndicator = async () => {
            const indicator = extensionRecords.find(record =>
                record.uuid === APP_NAME_INDICATOR
            );
            if (!indicator) {
                extensionStatus = "App Name Indicator is not installed in this profile.";
                updateExtensionStatus();
                return;
            }
            const operation = indicator.enabled ? "disable" : "enable";
            try {
                await services.extensions[operation]({ uuid: indicator.uuid });
                await refreshExtensions();
                extensionStatus = `App Name Indicator ${operation}d through the protected extension service.`;
            } catch (error) {
                extensionStatus = `${operation} failed: ${String(error?.message || error)}`;
            }
            updateExtensionStatus();
        };
        const callPaddy = async () => {
            try {
                const [reply] = await services.extensions.call({
                    uuid: "rice-paddy@ricewm.local",
                    objectPath: "/org/ricewm/Paddy",
                    interfaceName: "org.ricewm.Paddy",
                    method: "Echo",
                    args: ["Hello from the RiceApp SDK Guide"],
                });
                extensionStatus = reply;
                console.log("RiceApp → GNOME extension D-Bus reply", reply);
            } catch (error) {
                extensionStatus = `D-Bus call failed: ${String(error?.message || error)}`;
            }
            updateExtensionStatus();
        };
        const codeCard = (title, code, explanation) => new Container({
            text: "panel:fixed",
            state: enabled | State.LIQUID_GLASS | State.INFORMATION,
        }, [
            new Row({ text: "panel:trailing-action" }, [
                new Label({ text: title, state: enabled | State.EMPHASIZED }),
                activate(new Button({ text: "Copy", icon: 55 }), () => copy(code)),
            ]),
            new Markdown({ text: `\`\`\`js\n${code}\n\`\`\`\n\n${explanation}` }),
        ]);

        async function ensureStarter() {
            if ((await services.files.stat({ path: projectDirectory })).kind !== 2) {
                await services.files.mkdir({ path: projectDirectory });
            }
            if ((await services.files.stat({ path: `${projectDirectory}/manifest.json` })).kind !== 1) {
                await services.files.write({
                    path: `${projectDirectory}/manifest.json`,
                    data: starterManifest,
                });
            }
            if ((await services.files.stat({ path: `${projectDirectory}/app.mjs` })).kind !== 1) {
                await services.files.write({
                    path: `${projectDirectory}/app.mjs`,
                    data: starterSource,
                });
            }
            if ((await services.files.stat({ path: `${projectDirectory}/counter-message.mjs` })).kind !== 1) {
                await services.files.write({
                    path: `${projectDirectory}/counter-message.mjs`,
                    data: starterCounterModule,
                });
            }
            status = "Starter ready. Open app.mjs or run it with live reload.";
            updateLiveStatus();
        }

        const startPage = () => [
            new Label({ text: "Build a RiceWM app in minutes", state: enabled | State.DISPLAY }),
            new Label({
                text: "RiceApps are ordinary JavaScript modules plus a small manifest. This guide has already made a safe editable project for you.",
            }),
            new Container({ text: "panel:fixed", state: enabled | State.LIQUID_GLASS | State.SUCCESS }, [
                new Label({ text: "Your first development loop", state: enabled | State.EMPHASIZED }),
                new Label({
                    text: "The starter is already on your RiceWM drive. Open it, run it with the watcher, edit any module, and save. A failed edit leaves the last good app running.",
                }),
                action("Run + prove one save", () => void (async () => {
                    console.info("development self-test activated");
                    const result = await services.applications.develop({
                        path: projectDirectory,
                    });
                    demoRevision += 1;
                    const changed = starterSource.replace(
                        "Hello from JavaScript!",
                        `Hello after save ${demoRevision}!`,
                    );
                    await services.files.write({
                        path: `${projectDirectory}/app.mjs`,
                        data: changed,
                    });
                    status = `${result.name} started; a real VFS save was sent to the watcher.`;
                    updateLiveStatus();
                })().catch(error => fail("Live reload self-test failed", error)), 32, true),
                new Label({ text: "1 · Open source   2 · Run with live reload   3 · Change code   4 · Save and watch the app restart in place" }),
                new Row({ text: "panel:welcome-actions" }, [
                    action("Open app.mjs in RicePaper", () => void services.applications.editSource({
                        path: projectDirectory,
                        relative: "app.mjs",
                    }).catch(error => fail("Could not open RicePaper", error)), 61),
                    action("Open manifest.json", () => void services.applications.editSource({
                        path: projectDirectory,
                        relative: "manifest.json",
                    }).catch(error => fail("Could not open RicePaper", error)), 61),
                    action("Open imported module", () => void services.applications.editSource({
                        path: projectDirectory,
                        relative: "counter-message.mjs",
                    }).catch(error => fail("Could not open RicePaper", error)), 61),
                ]),
                new Row({ text: "panel:welcome-actions" }, [
                    action("Open app + code + live log", () => void services.applications
                        .openDevelopmentWorkspace({
                            path: projectDirectory,
                            relative: "app.mjs",
                        })
                        .then(result => {
                            status = `Opening the three-window desk. Logs: ${result.logPath}`;
                            updateLiveStatus();
                        })
                        .catch(error => fail("Could not open development desk", error)), 32, true),
                    action("Run with live reload", () => void services.applications.develop({
                        path: projectDirectory,
                    }).then(result => {
                        status = `${result.name} is installed, running, and watched for saves.`;
                        updateLiveStatus();
                    }).catch(error => fail("Development launch failed", error)), 32, true),
                    activate(new Button({ text: "Stop watching" }), () => void services.applications
                        .stopDevelopment({ id: "dev.example.hello-ricewm" })
                        .then(() => {
                            status = "Live reload stopped; the installed app remains in the Rice menu.";
                            updateLiveStatus();
                        })
                        .catch(error => fail("Could not stop live reload", error))),
                ]),
            ]),
            (statusLabel = new Label({ text: status, state: enabled | State.INFORMATION })),
            (reloadLabel = new Label({
                text: `Successful automatic reloads: ${reloads}`,
                state: enabled | State.MUTED,
            })),
        ];

        const controlsPage = () => [
            new Label({ text: "Controls, messages, and events", state: enabled | State.DISPLAY }),
            new Label({
                text: "Every control inherits from Control. Families add shared behavior: containers own children, inputs expose values, commands activate, and collections select items.",
            }),
            codeCard(
                "Create retained controls",
                "const label = new Label({ text: \"Ready\" });\nconst button = new Button({ text: \"Run\" });\nbutton.on(Message.ACTIVATE, () => { label.text = \"Done\"; });",
                "Changing `label.text` schedules a bounded retained patch. You do not rebuild a DOM or implement hit testing.",
            ),
            codeCard(
                "Cross-app RiceBus",
                "bus.subscribe(\"dev.example/tasks/\", event => console.log(event.payload));\nbus.publish(\"dev.example/tasks/changed\", { count: 1 });",
                "Topics and services are manifest-allowlisted. Payloads are cloned, size-limited plain data.",
            ),
            codeCard(
                "Native scrolling",
                "new ScrollView({}, [new Column({}, lotsOfControls)])",
                "Wheel, touch, keyboard, scrollbar thumb, and retained position are RiceUI behavior—not application event code.",
            ),
        ];

        const modulesPage = () => [
            new Label({ text: "Split an app into small modules", state: enabled | State.DISPLAY }),
            new Label({
                text: "A .riceapp contains the complete project tree, not only app.mjs. Relative imports stay inside the signed package boundary and never fall through to browser fetch.",
            }),
            codeCard(
                "Import another package file",
                'import { counterMessage } from "./counter-message.mjs";\noutput.text = counterMessage(count);',
                "Use explicit `.mjs` extensions. Nested paths and `../` are accepted only when they remain inside the package. Dynamic import, HTTP imports, and import.meta remain blocked.",
            ),
            codeCard(
                "Export a focused helper",
                'export function counterMessage(count) {\n    return `Count: ${count}`;\n}',
                "RicePaper recognizes local import specifiers and can open the target in another tab, so the project remains easy to navigate.",
            ),
            new Label({
                text: "Tip: hover the quoted ./counter-message.mjs path in app.mjs. RicePaper shows an Open in new tab action without moving your caret.",
                state: enabled | State.INFORMATION,
            }),
            action("Open counter-message.mjs", () => void services.applications.editSource({
                path: projectDirectory,
                relative: "counter-message.mjs",
            }).catch(error => fail("Could not open imported module", error)), 61),
        ];

        const extensionsPage = () => [
            new Label({ text: "GNOME extension development", state: enabled | State.DISPLAY }),
            new Label({
                text: "RiceApps and ordinary GJS extensions can call each other without changing the GNOME extension SDK. RiceWM exposes a narrow, permission-declared D-Bus bridge instead of a proprietary global.",
            }),
            new Container({
                text: "panel:fixed",
                state: enabled | State.LIQUID_GLASS | State.SUCCESS,
            }, [
                new Label({ text: "Try the protected bridge", state: enabled | State.EMPHASIZED }),
                new Row({ text: "panel:welcome-actions" }, [
                    (extensionToggleButton = action(
                        "Enable App Name Indicator",
                        () => void toggleAppNameIndicator(),
                    )),
                    action("Refresh extensions", () => void refreshExtensions(), 80),
                    action("Open Extension Manager", () => void services.extensions
                        .openManager().catch(error => {
                            extensionStatus = `Could not open manager: ${String(error?.message || error)}`;
                            updateExtensionStatus();
                        }), 41),
                ]),
                action("RiceApp → Paddy: Echo", () => void callPaddy(), null, true),
                (extensionStatusLabel = new Label({
                    text: extensionStatus,
                    state: enabled | State.INFORMATION,
                })),
            ]),
            codeCard(
                "RiceApp lifecycle calls",
                `const installed = await services.extensions.list();\nawait services.extensions.enable({ uuid: "${APP_NAME_INDICATOR}" });\nawait services.extensions.restart({ uuid });`,
                "Each operation needs its exact manifest permission. Apps cannot read extension source or access the extension host directly.",
            ),
            new Container({
                text: "panel:fixed-height:620",
                state: enabled | State.LIQUID_GLASS | State.INFORMATION,
            }, [
                new Label({ text: "Two-way calls use ordinary GJS D-Bus", state: enabled | State.EMPHASIZED }),
                new Markdown({ text: `The extension uses standard \`Gio.DBusProxy\` and \`Gio.DBusExportedObject\` APIs:

\`\`\`js
${extensionBridgeExample}
\`\`\`

The receiving RiceApp declares \`bus.provide\` and handles the service with \`bus.expose(...)\`. RiceWM validates the destination, extension UUID, interface, method, JSON payload, enabled state, declared ownership, and 64 KiB message limit.` }),
            ]),
        ];

        const debugPage = () => [
            new Label({ text: "Debug without weakening the sandbox", state: enabled | State.DISPLAY }),
            new Markdown({
                text: "Use `console.log`, `console.info`, `console.warn`, `console.error`, timers, or the structured `log` helper. Records appear in browser DevTools and in a bounded per-app JSON-lines file. Arguments are cloned, bounded, and redacted before leaving the worker.\n\n```js\nconsole.time(\"load\");\nconsole.log(\"items loaded\", { count: items.length });\nconsole.timeEnd(\"load\");\n```\n\nTail this starter—including reload failures—from Rice Terminal:\n\n```sh\ntail -n 200 -F /var/log/riceapps/dev.example.hello-ricewm.log\n```\n\nThe **Open app + code + live log** action starts that tail automatically and tiles the app at upper left, log at lower left, and RicePaper on the right.",
            }),
            new Container({ text: "panel:fixed", state: enabled | State.LIQUID_GLASS | State.WARNING }, [
                new Label({ text: "When a save does not relaunch", state: enabled | State.EMPHASIZED }),
                new Label({
                    text: "The last good app stays available. This guide receives the compiler error over RiceBus; syntax and permission failures also appear in DevTools.",
                }),
            ]),
        ];

        const distributePage = () => [
            new Label({ text: "Install and distribute", state: enabled | State.DISPLAY }),
            new Markdown({
                text: "### Learn from complete apps\n\nOpen **Development → Examples → Calc source code** or **Development → Examples → Welcome source code** in the Rice menu. Each folder shortcut opens the complete editable project directory, including its manifest and every local module.\n\n### Package a project\n\nFrom a repository checkout, run:\n\n```sh\nnode tools/riceapp-pack.mjs my-app dist/my-app.riceapp\n```\n\n### Install it\n\n- Drop the `.riceapp` on the RiceWM desktop; RiceWM validates and installs it.\n- Or download it in RiceWM and open it. It immediately appears in the Rice menu.\n- Right-click its menu entry and choose **Uninstall** to remove only files owned by that package.\n\n### Publish it on GitHub\n\nCommit `manifest.json`, `app.mjs`, a README, screenshots, and a LICENSE. Attach the generated `.riceapp` to a GitHub Release and include its SHA-256 checksum. Never ask users to paste source into DevTools or grant blanket VFS/network access.",
            }),
            action("Copy packaging command", () => copy(
                "node tools/riceapp-pack.mjs my-app dist/my-app.riceapp",
            ), 55),
        ];

        const sections = [
            { name: "Start here", anchor: "guide-start", threshold: 0, build: startPage },
            { name: "Modules", anchor: "guide-modules", threshold: 0.15, build: modulesPage },
            { name: "Controls & bus", anchor: "guide-controls", threshold: 0.33, build: controlsPage },
            { name: "GNOME extensions", anchor: "guide-extensions", threshold: 0.49, build: extensionsPage },
            { name: "Debugging", anchor: "guide-debugging", threshold: 0.73, build: debugPage },
            { name: "Install & share", anchor: "guide-install", threshold: 0.88, build: distributePage },
        ];

        const selectSection = (name, { scrollTo = false } = {}) => {
            activeSection = name;
            for (const [candidate, button] of navigationButtons) {
                const active = candidate === name;
                button.setProperty("selected", active);
                button.setProperty("accent", active);
            }
            if (readingLabel) readingLabel.text = `Reading: ${name}`;
            if (scrollTo) {
                const anchor = sectionAnchors.get(name);
                if (anchor?.id) documentScroll.text = `anchor:${anchor.id}`;
            }
        };

        const navigation = new Column({ text: "panel:welcome-navigation" });
        for (const section of sections) {
            const button = activate(new Button({
                text: section.name,
                state: enabled | State.START_ALIGNED | State.FLAT,
                selected: section.name === activeSection,
                accent: section.name === activeSection,
            }), () => selectSection(section.name, { scrollTo: true }));
            navigationButtons.set(section.name, button);
            navigation.append(button);
        }
        readingLabel = new Label({
            text: `Reading: ${activeSection}`,
            state: enabled | State.MUTED,
        });
        navigation.append(readingLabel);

        const documentChildren = [];
        for (const [index, section] of sections.entries()) {
            const anchor = new Column({
                name: section.anchor,
                text: "panel:clamp:820:section-stack",
            }, section.build());
            sectionAnchors.set(section.name, anchor);
            documentChildren.push(anchor);
            if (index + 1 < sections.length) {
                documentChildren.push(new Label({ text: "panel:document-gap" }));
            }
        }
        documentScroll = new ScrollView({ name: "guide-document-scroll", text: "0/0" }, [
            new Column({ text: "panel:welcome-document" }, documentChildren),
        ]);
        documentScroll.on(Message.VALUE_CHANGED, event => {
            const match = /^(\d+)\/(\d+)$/u.exec(String(event.detail?.value || ""));
            if (!match) return;
            const maximum = Number(match[2]);
            const ratio = maximum > 0 ? Number(match[1]) / maximum : 0;
            let current = sections[0];
            for (const section of sections) {
                if (ratio + Number.EPSILON >= section.threshold) current = section;
            }
            if (current.name !== activeSection) selectSection(current.name);
        });

        const root = new SplitPane({ text: "split:190:min-width=170:max-width=250" }, [
            navigation,
            new Column({}, [documentScroll]),
        ]);
        window = createWindow({ title: "RiceApp SDK Guide", root });
        bus.subscribe("org.ricewm.development/status", event => {
            if (event.payload?.id !== "dev.example.hello-ricewm") return;
            reloads = Number(event.payload.reloads) || reloads;
            status = event.payload.state === "error"
                ? `Build error: ${event.payload.message}`
                : `Live development: ${event.payload.state}.`;
            updateLiveStatus();
        });
        bus.expose("org.ricewm.extension/demo", event => {
            extensionStatus = `${event.source}: ${JSON.stringify(event.request)}`;
            console.log("GNOME extension → RiceApp D-Bus call", event);
            updateExtensionStatus();
            return {
                ok: true,
                message: "RiceApp SDK Guide received the extension call",
            };
        });
        await ensureStarter();
        await refreshExtensions();
        console.log("RiceApp SDK Guide ready", { projectDirectory });
    },
});

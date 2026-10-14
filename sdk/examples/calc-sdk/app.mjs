import {
    Button, Column, Container, Dialog, Label, Menu, MenuBar, MenuItem, Message,
    RadioButton, Row, ScrollView, SplitPane, State, TextField, defineApp,
} from "@ricewm/app";

const enabled = State.ENABLED;
const BASIC_ROWS = [
    [["(", "("], [")", ")"], ["%", "%"], ["÷", "/"]],
    [["7", "7"], ["8", "8"], ["9", "9"], ["×", "*"]],
    [["4", "4"], ["5", "5"], ["6", "6"], ["−", "-"]],
    [["1", "1"], ["2", "2"], ["3", "3"], ["+", "+"]],
    [["0", "0"], [".", "."], ["π", "pi"], ["=", "="]],
];
const ADVANCED_ROWS = [
    [["sin", "sin("], ["cos", "cos("], ["tan", "tan("], ["√", "sqrt("], ["ln", "ln("]],
    [["log", "log("], ["x!", "!"], ["xʸ", "^"], ["e", "e"]],
];

function calculate(source, degrees) {
    const input = String(source).replace(/×/gu, "*").replace(/÷/gu, "/").replace(/−/gu, "-");
    const tokens = input.match(/(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?|[A-Za-z]+|[()+\-*/%^!]/giu) || [];
    if (tokens.join("").toLowerCase() !== input.replace(/\s+/gu, "").toLowerCase()) {
        throw new Error("Unsupported expression");
    }
    let at = 0;
    const angle = value => degrees ? value * Math.PI / 180 : value;
    const primary = () => {
        const token = tokens[at++];
        if (token === "(") {
            const value = expression();
            if (tokens[at++] !== ")") throw new Error("Missing )");
            return value;
        }
        if (token === "+") return primary();
        if (token === "-") return -primary();
        if (/^[A-Za-z]+$/u.test(token || "")) {
            const name = token.toLowerCase();
            if (name === "pi") return Math.PI;
            if (name === "e") return Math.E;
            if (tokens[at++] !== "(") throw new Error(`${name} needs parentheses`);
            const value = expression();
            if (tokens[at++] !== ")") throw new Error("Missing )");
            const functions = {
                sqrt: Math.sqrt,
                sin: value => Math.sin(angle(value)),
                cos: value => Math.cos(angle(value)),
                tan: value => Math.tan(angle(value)),
                log: Math.log10,
                ln: Math.log,
                abs: Math.abs,
            };
            if (!functions[name]) throw new Error(`Unknown function ${name}`);
            return functions[name](value);
        }
        const value = Number(token);
        if (!Number.isFinite(value)) throw new Error("Number expected");
        return value;
    };
    const factorial = () => {
        let value = primary();
        while (tokens[at] === "!") {
            at += 1;
            if (!Number.isInteger(value) || value < 0 || value > 170) throw new Error("Factorial needs an integer from 0 to 170");
            let result = 1;
            for (let number = 2; number <= value; number += 1) result *= number;
            value = result;
        }
        return value;
    };
    const power = () => {
        let value = factorial();
        while (tokens[at] === "^") {
            at += 1;
            value **= power();
        }
        return value;
    };
    const product = () => {
        let value = power();
        while (["*", "/", "%"].includes(tokens[at])) {
            const operator = tokens[at++];
            const right = power();
            value = operator === "*" ? value * right : operator === "/" ? value / right : value % right;
        }
        return value;
    };
    const expression = () => {
        let value = product();
        while (["+", "-"].includes(tokens[at])) {
            const operator = tokens[at++];
            const right = product();
            value = operator === "+" ? value + right : value - right;
        }
        return value;
    };
    const result = expression();
    if (at !== tokens.length || !Number.isFinite(result)) throw new Error("Expression has no finite result");
    return Number.parseFloat(result.toPrecision(14)).toString();
}

export default defineApp({
    manifest: { id: "dev.ricewm.calc-sdk", name: "Calc", version: "2.0.0" },
    start({ app, log, services, createWindow }) {
        const model = {
            expression: "", result: "", error: "", memory: 0, mode: "Basic",
            degrees: true, historyVisible: false, history: [], dialog: null,
        };
        let window;
        const activate = (control, action) => {
            control.on(Message.ACTIVATE, action);
            return control;
        };
        const preview = () => {
            model.error = "";
            if (!model.expression) {
                model.result = "";
                return;
            }
            try { model.result = calculate(model.expression, model.degrees); }
            catch { model.result = ""; }
        };
        const append = value => {
            if (value === "=") return evaluate();
            model.expression += value;
            preview();
            render();
        };
        const evaluate = () => {
            if (!model.expression) return;
            try {
                const result = calculate(model.expression, model.degrees);
                model.history.unshift([model.expression, result]);
                model.history = model.history.slice(0, 50);
                model.result = result;
                model.expression = result;
                model.error = "";
                log.info("calculation", { result });
            } catch (error) {
                model.error = String(error?.message || error);
                model.result = model.error;
            }
            render();
        };
        const clear = () => {
            model.expression = "";
            model.result = "";
            model.error = "";
            render();
        };
        const backspace = () => {
            model.expression = model.expression.slice(0, -1);
            preview();
            render();
        };
        const menuItem = (label, shortcut, action) => activate(new MenuItem({
            text: shortcut ? `${label}\t${shortcut}` : label,
        }), action);
        const radioItem = (label, checked, action) => activate(new RadioButton({
            text: label,
            state: enabled | (checked ? State.CHECKED : 0),
            checked,
        }), action);
        const separator = () => new MenuItem({ text: "menu:separator", enabled: false, state: 0 });
        const menus = () => {
            const mode = new Menu({ text: "Mode" }, [
                ...["Basic", "Advanced", "Programmer", "Financial", "Conversion"].map(name =>
                    radioItem(name, model.mode === name, () => {
                        model.mode = name;
                        render();
                    })),
                separator(),
            ]);
            const edit = new Menu({ text: "Edit" }, [
                menuItem("Copy Result", "Alt+C", () => void services.clipboard.writeText(model.result || model.expression)),
                menuItem("Clear", "Escape", clear),
                menuItem("Backspace", "Backspace", backspace),
            ]);
            const viewChildren = [
                menuItem(model.historyVisible ? "Hide History" : "Show History", "Alt+H", () => {
                    model.historyVisible = !model.historyVisible;
                    render();
                }),
                menuItem("Clear History", null, () => {
                    model.history = [];
                    render();
                }),
            ];
            if (model.mode === "Advanced") {
                viewChildren.push(separator());
                viewChildren.push(radioItem("Degrees", model.degrees, () => {
                    model.degrees = true;
                    preview();
                    render();
                }));
                viewChildren.push(radioItem("Radians", !model.degrees, () => {
                    model.degrees = false;
                    preview();
                    render();
                }));
            }
            return new MenuBar({}, [
                mode,
                edit,
                new Menu({ text: "View" }, viewChildren),
                new Menu({ text: "Help" }, [
                    menuItem("Keyboard Shortcuts", "Alt+?", () => {
                        model.dialog = "shortcuts";
                        render();
                    }),
                    menuItem("About RiceCalc", null, () => {
                        model.dialog = "about";
                        render();
                    }),
                ]),
            ]);
        };
        const keypadButton = ([label, value]) => activate(new Button({
            text: label,
            state: enabled |
                (value === "=" ? State.ACCENT : 0) |
                (["/", "*", "-", "+", "%", "^"].includes(value) ? State.EMPHASIZED : 0),
            accent: value === "=",
        }), () => append(value));
        const buttonRow = entries => new Row({ text: "panel:equal" }, entries.map(keypadButton));
        const memoryRow = () => new Row({ text: "panel:equal-fixed" }, [
            activate(new Button({ text: "MC" }), () => { model.memory = 0; }),
            activate(new Button({ text: "MR" }), () => append(String(model.memory))),
            activate(new Button({ text: "MS" }), () => { model.memory = Number(model.result) || 0; }),
            activate(new Button({ text: "M+" }), () => { model.memory += Number(model.result) || 0; }),
            activate(new Button({ text: "M−" }), () => { model.memory -= Number(model.result) || 0; }),
            activate(new Button({ text: "C", destructive: true }), clear),
            activate(new Button({ text: "←" }), backspace),
        ]);
        const worksheet = title => {
            const fields = title === "Financial"
                ? ["Principal / cost", "Periodic interest rate", "Number of periods", "Salvage value", "Future value / current period"]
                : ["Amount", "From unit", "To unit"];
            return new Column({ text: "panel:worksheet" }, [
                new Label({ text: title, state: enabled | State.EMPHASIZED }),
                ...fields.map(label => new Row({}, [new Label({ text: label }), new TextField({ value: "" })])),
                new Row({}, [new Button({ text: title === "Financial" ? "Future Value" : "Convert units", accent: true })]),
                new Label({ text: "Choose values and an operation", state: enabled | State.MUTED }),
            ]);
        };
        const keypad = () => {
            if (model.mode === "Financial" || model.mode === "Conversion") {
                return worksheet(model.mode === "Financial" ? "Financial" : "Convert");
            }
            const children = [memoryRow()];
            if (model.mode === "Advanced") children.push(...ADVANCED_ROWS.map(buttonRow));
            if (model.mode === "Programmer") {
                children.push(buttonRow([["AND", " and "], ["OR", " or "], ["XOR", " xor "], ["<<", " << "], [">>", " >> "], ["mod", " mod "]]));
            }
            children.push(...BASIC_ROWS.map(buttonRow));
            return new Column({ text: model.mode === "Programmer" ? null : "panel:distribute" }, children);
        };
        const history = () => new ScrollView({ text: "panel:history" }, [
            new Label({ text: "History", state: enabled | State.EMPHASIZED }),
            ...(model.history.length
                ? model.history.map(([expression, result]) => activate(new Button({ text: `${expression}\n= ${result}` }), () => {
                    model.expression = expression;
                    model.result = result;
                    render();
                }))
                : [new Label({ text: "No calculations yet", state: enabled | State.MUTED })]),
        ]);
        const dialog = () => {
            if (!model.dialog) return null;
            const title = model.dialog === "about" ? "About RiceCalc" : "Keyboard Shortcuts";
            const body = model.dialog === "about"
                ? "RiceCalc\nA dependency-free calculator for RiceWM.\nArbitrary-precision arithmetic, scientific, financial, conversion and programmer tools."
                : "Enter — calculate\nEscape — clear\nBackspace — delete previous character\nAlt+C — copy result\nAlt+H — show or hide history\nAlt+? — keyboard shortcuts";
            // Keep the same dialog layout token and retained tree as the
            // original calculator, so About and Shortcuts are visually
            // indistinguishable after the SDK migration.
            return new Dialog({
                text: "panel:dialog",
                state: enabled,
                open: true,
                modal: true,
            }, [
                new Label({ text: title, state: enabled | State.EMPHASIZED }),
                new Label({ text: body }),
                new Row({ text: "panel:actions" }, [activate(new Button({ text: "Close", accent: true }), () => {
                    model.dialog = null;
                    render();
                })]),
            ]);
        };
        const build = () => {
            const expression = new TextField({
                value: model.expression,
                state: enabled | State.FOCUSED | State.END_ALIGNED | State.FLAT |
                    (model.expression ? State.MUTED : State.DISPLAY) |
                    (model.error ? State.INVALID : 0),
                focused: true,
            });
            expression.on(Message.VALUE_CHANGED, event => {
                model.expression = String(event.detail?.value ?? expression.getProperty("value"));
                preview();
                render();
            });
            expression.on(Message.KEY, event => {
                if (!event.detail?.pressed) return;
                const key = event.detail?.key;
                if (key === "Enter") evaluate();
                else if (key === "Escape") clear();
                else if (key === "Backspace") backspace();
            });
            const displayChildren = [expression];
            if (model.expression) displayChildren.push(new Label({
                text: model.result,
                state: enabled | State.DISPLAY | State.END_ALIGNED | (model.error ? State.DESTRUCTIVE : 0),
            }));
            const bodyChildren = [keypad()];
            if (model.historyVisible) bodyChildren.push(history());
            const children = [
                menus(),
                new Container({ text: "panel:fixed" }, displayChildren),
                new SplitPane({ text: "split:700" }, bodyChildren),
            ];
            const overlay = dialog();
            if (overlay) children.push(overlay);
            return new Column({ text: "panel:clamp:520" }, children);
        };
        function render() {
            window.setRoot(build());
        }
        window = createWindow({ title: "Calc", root: build() });
        app.on(Message.APP_CLOSE, () => log.info("closed"));
    },
});

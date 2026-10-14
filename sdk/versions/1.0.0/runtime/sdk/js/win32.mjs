import { CollectionControl } from "./control.mjs";
import {
    Button, Label, List, ProgressBar, Row, ScrollBar, ScrollView, Slider, Spinner, Tabs,
    TextEditor, TextField, ToolBar, Tooltip, Tree,
} from "./controls.mjs";
import {
    ComboBox, CommandLink, Link, SpinControl, SplitButton, StatusBar,
} from "./composites.mjs";
import { SystemDialogs } from "./services.mjs";
import { dateValue } from './date-time-values.mjs';
import { DatePicker } from './date-time-picker.mjs';
export { Calendar, DatePicker, TimeEditor, TimePicker, TimeSelector } from './date-time-picker.mjs';

// Names follow Win32 standard/common-control terminology. They remain normal
// RiceUI controls and do not expose HWNDs, guest pointers, or owner-draw hooks.
export class StaticControl extends Label {}
export class EditControl extends TextField {}
export class RichEditControl extends TextEditor {}
export class PushButton extends Button {}
export class ListBoxControl extends List {}
export class ComboBoxControl extends ComboBox {}
export class ScrollBarControl extends ScrollBar {}
export class HeaderControl extends Row {}
export class ListViewControl extends List {}
export class TreeViewControl extends Tree {}
export class TabControl extends Tabs {}
export class ToolbarControl extends ToolBar {}
export class StatusBarControl extends StatusBar {}
export class TrackbarControl extends Slider {}
export class ProgressControl extends ProgressBar {}
export class UpDownControl extends SpinControl {}
export class TooltipControl extends Tooltip {}
export class RebarControl extends Row {}
export class ComboBoxExControl extends ComboBox {}
export class PagerControl extends ScrollView {}
export class SysLinkControl extends Link {}
export class CommandLinkControl extends CommandLink {}
export class SplitButtonControl extends SplitButton {}
export class AnimationControl extends Spinner {}

export class MonthCalendar extends CollectionControl {
    static properties = Object.freeze({
        value: { default: "", validate: dateValue, reflect: false },
        firstDayOfWeek: { default: "locale", validate: value => String(value), reflect: false },
    });
}
export class IpAddressControl extends TextField {
    static properties = Object.freeze({
        value: {
            default: "",
            validate: value => {
                const text = String(value || "");
                if (text && !/^(?:\d{1,3}\.){3}\d{1,3}$/u.test(text)) throw new TypeError("IP address is invalid");
                if (text && text.split(".").some(part => Number(part) > 255)) throw new RangeError("IP address octet is invalid");
                return text;
            },
        },
    });
}
export class HotKeyControl extends TextField {
    static properties = Object.freeze({
        value: { default: "", validate: value => String(value || "").slice(0, 128) },
        capture: { default: true, validate: value => Boolean(value), reflect: false },
    });
}

export class Win32SystemDialogs extends SystemDialogs {
    messageBox(text, title = "Message", kind = "information") {
        return this.message({ text, title, kind });
    }
    taskDialog(options) { return this.confirm(options); }
    getOpenFileName(options) { return this.openFile(options); }
    getSaveFileName(options) { return this.saveFile(options); }
    browseForFolder(options) { return this.chooseFolder(options); }
    chooseColorDialog(options) { return this.chooseColor(options); }
    chooseFontDialog(options) { return this.chooseFont(options); }
    printDialog(options) { return this.print(options); }
    pageSetupDialog(options) { return this.pageSetup(options); }
    findTextDialog(options) { return this.find(options); }
    replaceTextDialog(options) { return this.replace(options); }
}

export const WIN32_CONTROL_EQUIVALENTS = Object.freeze({
    STATIC: StaticControl,
    EDIT: EditControl,
    RICHEDIT: RichEditControl,
    BUTTON: PushButton,
    LISTBOX: ListBoxControl,
    COMBOBOX: ComboBoxControl,
    SCROLLBAR: ScrollBarControl,
    HEADER: HeaderControl,
    LISTVIEW: ListViewControl,
    TREEVIEW: TreeViewControl,
    TAB: TabControl,
    TOOLBAR: ToolbarControl,
    STATUSBAR: StatusBarControl,
    TRACKBAR: TrackbarControl,
    PROGRESS: ProgressControl,
    UPDOWN: UpDownControl,
    TOOLTIPS: TooltipControl,
    REBAR: RebarControl,
    COMBOBOXEX: ComboBoxExControl,
    DATETIMEPICK: DatePicker,
    MONTHCAL: MonthCalendar,
    IPADDRESS: IpAddressControl,
    HOTKEY: HotKeyControl,
    PAGER: PagerControl,
    SYSLINK: SysLinkControl,
    ANIMATE: AnimationControl,
    COMMANDLINK: CommandLinkControl,
    SPLITBUTTON: SplitButtonControl,
});

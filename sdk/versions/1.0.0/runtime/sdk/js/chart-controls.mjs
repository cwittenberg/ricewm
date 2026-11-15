import {CONTROL_CLASSES as primitives,ui as primitiveUi} from './controls.mjs';
import {Chart} from './charts.mjs';
import {Table,DataTable} from './data-table.mjs';
import {Chip,Badge,ChipGroup,TokenInput} from './chips.mjs';
import {ObjectCanvas} from './object-canvas.mjs';
import {RichTextEditor} from './rich-text.mjs';
import {ObjectToolbar,ObjectInspector} from './object-controls.mjs';
import {WrapPanel} from './wrap-panel.mjs';
import {Ribbon} from './ribbon.mjs';
import {ColorPalette,ColorMenu} from './color-palette.mjs';
import {TabView,TabPage} from './tab-view.mjs';
import {Form,FormField} from './forms.mjs';
import {Wizard,WizardStep} from './wizard.mjs';
import {Gallery,PropertyInspector} from './property-inspector.mjs';
import {DocumentEditor} from './document-editor-control.mjs';
import {EditableTable} from './editable-collection.mjs';
import {NumberInput,EditableChoice,ColorInput} from './value-editors.mjs';
import {HexColorInput} from './hex-color-input.mjs';
import {GradientEditor} from './gradient-editor.mjs';
import {GeometryEditor} from './geometry-editor.mjs';
import {ThumbnailTile} from './thumbnail-tile.mjs';
import {SectionHeader} from './section-header.mjs';
import {ReorderButton} from './reorder-controller.mjs';
import {CommandPalette} from './command-palette.mjs';
import {EditorWorkspace} from './editor-workspace.mjs';
import {SceneTextEditor} from './scene-text-editor.mjs';
import {SceneHandleOverlay} from './scene-overlay.mjs';
import {SplitButton,DropDownList,ComboBox} from './composites.mjs';

const visualControls=Object.freeze({Chip,Badge,ChipGroup,TokenInput,ObjectCanvas,RichTextEditor,ObjectToolbar,ObjectInspector,WrapPanel,Ribbon,ColorPalette,ColorMenu,SplitButton,DropDownList,ComboBox,TabView,TabPage,Form,FormField,Wizard,WizardStep,Gallery,PropertyInspector,DocumentEditor,EditableTable,NumberInput,EditableChoice,ColorInput,HexColorInput,GradientEditor,GeometryEditor,ThumbnailTile,SectionHeader,ReorderButton,CommandPalette,EditorWorkspace,SceneTextEditor,SceneHandleOverlay});

/** Factories and reflection share the same actual constructors. */
export const CONTROL_CLASSES=Object.freeze({...primitives,Chart,Table,DataTable,...visualControls});
export function createControl(type,properties={},children=[]) {
    const Constructor=CONTROL_CLASSES[type];
    if(!Constructor)throw TypeError(`unknown RiceUI control ${type}`);
    if(type==='Chart'&&children.length)throw TypeError('Chart owns its native children; supply data/spec or option');
    return new Constructor(properties,children);
}
export const ui=Object.freeze({...primitiveUi,chart:(properties={})=>new Chart(properties),table:(properties={},children=[])=>new Table(properties,children),dataTable:(properties={})=>new DataTable(properties),...Object.fromEntries(Object.entries(visualControls).map(([name,Constructor])=>[name[0].toLowerCase()+name.slice(1),(properties={},children=[])=>new Constructor(properties,children)]))});

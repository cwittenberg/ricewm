import { Container, HoldMenuButton, Menu, MenuBar, Ribbon, Row, State, ToolBar, Tooltip } from '@ricewm/app';

// Semantic RiceUI icon IDs: the host selects artwork and ink from its theme.
const icons = Object.freeze({
    'insert-chart':241,'chart-type':243,'insert-chart-column':241,'insert-chart-line':242,'format-chart':243,'chart-data':244,'delete-chart':25,
    new:10, open:11, save:12, 'save-as':67, 'save-as-xlsx':109, 'save-as-ods':109, 'save-as-csv':109, 'save-as-tsv':109,
    undo:20, redo:21, cut:22, copy:23, paste:24, 'format-painter':514, clear:25, find:30, about:51, exit:54,
    'font-bold':126, 'font-italic':127, 'font-underline':128, 'font-grow':158, 'font-shrink':157,
    'wrap-text':565,'vertical-top':233,'vertical-center':234,'vertical-bottom':235,'decimal-increase':566,'decimal-decrease':567,'remove-duplicates':568,'text-to-columns':569,
    'merge-cells':41, 'unmerge-cells':237, 'align-left':151, 'align-center':152, 'align-right':153,
    'toggle-filter':162, 'filter-values':162, 'clear-filters':163,
    sort:164, 'sort-ascending':164, 'sort-descending':165,
    'name-manager':109,'define-name':178,'use-name':166,functions:166, autosum:167, vlookup:168, 'fill-down':169, 'fill-right':170,
    'paste-values':171, 'paste-formulas':172, 'row-height':173, 'column-width':174,
    'format-general':175, 'format-number':175, 'format-percent':176, 'format-currency':177, 'format-date':88,
    'insert-rows':178, 'insert-columns':179, 'delete-rows':180, 'delete-columns':181,
    'create-sheet':76, 'rename-sheet':69, 'duplicate-sheet':23, 'move-sheet':32,
    'move-sheet-left':31, 'move-sheet-right':32, 'delete-sheet':25,
    'toggle-formula':166, 'toggle-status':51, 'format-cells':238, 'cell-styles':236, 'font-color':188, 'fill-color':186, borders:237, insert:178, delete:180, 'paste-special':171, 'clear-column-filter':163,
});

export function createSheetRibbon(options) {
    const { editorAction, fontAction, documentAction, structureAction, dataAction, action, toggle,
        fontSize, fontFamily, formulaBar, showAbout, sensitivityItem, sensitivityBadge, showFind, showFilter, clearFilters, exit,
        canUseSheetDataIcons = () => false, canSplit = () => false, canDecimal = () => false, canGrowFont = () => false, canShrinkFont = () => false, canClearFilters = () => false,
        canMerge = () => false, canUnmerge = () => false, canFill = () => true, canLayout = () => true, canMoveSheetLeft = () => true,
        canMoveSheetRight = () => true, canDeleteSheet = () => true, canEditChart = () => false } = options;
    const newIconControls=[];
    const iconFallbacks={'wrap-text':[151,'Wrap'],'decimal-increase':[175,'+0.0'],'decimal-decrease':[175,'−0.0'],'remove-duplicates':[25,'Remove Duplicates'],'text-to-columns':[179,'Text to Columns']};
    const decorate = (item, name, label) => {
        const fallback=iconFallbacks[name];
        item.setProperty('icon',fallback?fallback[0]:(icons[name]||(name.startsWith('functions-')?166:109)));
        if(fallback)newIconControls.push({item,name});
        item.setProperty('tooltip',label);item.setProperty('accessibleName',label);return item;
    };
    const prominent = (control, label) => {
        control.setProperty('state', control.getProperty('state') | State.START_ALIGNED);
        control.text = label; return control;
    };
    const edit = (name, label, capability, toolbar = true, available = null) =>
        decorate(editorAction(name, label, capability, toolbar, available), name, label);
    const doc = (name, label, toolbar = false) => decorate(documentAction(name, label, toolbar), name, label);
    const structure = (name, label, capability = name, toolbar = true, available = null) =>
        decorate(structureAction(name, label, capability, toolbar, available), name, label);
    const menu = (name, label, icon, children) => new Menu({ name, text: label, icon, tooltip: label, accessibleName: label }, children);
    const group = (id, title, controls, primary = 0, rowBreak = undefined) => ({id,title,controls,primary,rowBreak});
    const data = (name, label, callback) => decorate(dataAction(name, label, callback, true), name, label);
    const fill = () => menu('fill-menu', 'Fill', 169, [
        edit('fill-down', 'Fill Down (Ctrl+D)', 'fill-range', false, () => canFill('down')),
        edit('fill-right', 'Fill Right (Ctrl+R)', 'fill-range', false, () => canFill('right')),
    ]);
    const clipboard = group('clipboard', 'Clipboard', [edit('paste', 'Paste (Ctrl+V)', 'paste-range'),
        edit('copy', 'Copy (Ctrl+C)', 'copy-range'),edit('format-painter','Format Painter','paint-format'), menu('paste-special', 'Paste Special', 171, [
            edit('paste-values', 'Paste Values (Ctrl+Shift+V)', 'paste-range', false),
            edit('paste-formulas', 'Paste Formulas', 'paste-range', false),
        ])], 0, 2);
    const font = group('font', 'Font', [
        new Row({text:'panel:fixed:width=178:flush:gap=0'},[new Container({text:'panel:fixed:width=98:flush'},[fontFamily]),new Container({ text:'panel:fixed:width=80:flush' }, [fontSize])]),
        edit('font-grow','Increase Font Size','format-range',true,canGrowFont),edit('font-shrink','Decrease Font Size','format-range',true,canShrinkFont),
        ...[['font-bold', 'Bold (Ctrl+B)', 'weight'], ['font-italic', 'Italic (Ctrl+I)', 'italic'], ['font-underline', 'Underline (Ctrl+U)', 'underline']]
            .map(([name,label,property]) => decorate(fontAction(name,label,property,true),name,label)),
        edit('font-color','Text Color…','format-cells'),edit('fill-color','Fill Color…','format-cells'),edit('borders','Borders…','format-cells'),
    ], 0, 3);
    const alignment = group('alignment', 'Alignment', [
        edit('vertical-top','Top Align','format-cells'),edit('vertical-center','Middle Align','format-cells'),edit('vertical-bottom','Bottom Align','format-cells'),edit('wrap-text','Wrap Text','format-cells'),
        edit('align-left','Align Left','format-range'), edit('align-center','Align Center','format-range'), edit('align-right','Align Right','format-range'),
        edit('merge-cells','Merge Cells','merge-cells',true,canMerge),
        edit('unmerge-cells','Unmerge Cells','unmerge-cells',true,canUnmerge),
    ], 0, 4);
    const numberSelector = prominent(menu('number-format', 'Number Format', 175,
        [['general','General'],['number','Number'],['percent','Percentage'],['currency','Currency'],['date','Date']]
            .map(([name,label]) => edit(`format-${name}`,label,'format-range',false))), 'General');
    const number = group('number', 'Number', [new Container({text:'panel:fixed:width=124:flush'},[numberSelector]),
        edit('decimal-increase','Increase Decimal Places','format-range',true,()=>canDecimal(1)),edit('decimal-decrease','Decrease Decimal Places','format-range',true,()=>canDecimal(-1)),
        edit('format-cells','Format Cells… (Ctrl+1)','format-cells')],0,1);
    const cells = group('cells', 'Cells', [prominent(edit('cell-styles','Cell Styles','apply-cell-style'),'Styles'),menu('cell-layout', 'Cell Size and Structure', 174, [
        edit('row-height','Row Height…','resize-rows',false,canLayout), edit('column-width','Column Width…','resize-columns',false,canLayout),
        structure('insert-rows','Insert Rows Above','insert-rows',false), structure('insert-columns','Insert Columns Before','insert-columns',false),
        structure('delete-rows','Delete Selected Rows…','delete-rows',false), structure('delete-columns','Delete Selected Columns…','delete-columns',false),
    ])]);
    const editing = group('editing','Edit', [prominent(edit('autosum','AutoSum','edit-cell'),'AutoSum'),fill(),
        menu('sort-filter-menu','Sort & Filter',162,[edit('sort-ascending','Sort Ascending…','sort-range',false),edit('sort-descending','Sort Descending…','sort-range',false),edit('sort','Custom Sort…','sort-range',false),edit('toggle-filter','AutoFilter','toggle-autofilter',false),edit('home-clear-filters','Clear All Filters','toggle-autofilter',false,canClearFilters)]),
        edit('clear','Clear Contents (Delete)','clear-range'), data('find','Find (Ctrl+F)',showFind)],0,3);
    const page = (id, groups) => ({id,title:id[0].toUpperCase()+id.slice(1),groups});
    const pages = new Map([
        ['home',page('home',[clipboard,font,alignment,number,cells,editing])],
        ['insert',page('insert',[
            group('charts','Charts',[prominent(edit('insert-chart','Charts…','create-chart'),'Charts…'),prominent(edit('insert-chart-column','Column Chart','create-chart'),'Column Chart'),prominent(edit('insert-chart-line','Line Chart','create-chart'),'Line Chart')]),
            group('insert-cells','Rows and Columns',[structure('insert-rows','Insert Rows Above'),structure('insert-columns','Insert Columns Before')]),
            group('sheets','Sheets',[
                structure('create-sheet','New Sheet'), structure('rename-sheet','Rename Sheet…'), structure('duplicate-sheet','Duplicate Sheet'),
                menu('sheet-organize','Organize Sheets',109,[structure('move-sheet','Move Sheet…','move-sheet',false),
                    structure('move-sheet-left','Move Sheet Left','move-sheet',false,canMoveSheetLeft),
                    structure('move-sheet-right','Move Sheet Right','move-sheet',false,canMoveSheetRight),
                    structure('delete-sheet','Delete Sheet…','delete-sheet',false,canDeleteSheet)]),
            ]),
        ])],
        ['formulas',page('formulas',[
            group('functions','Functions',[prominent(edit('autosum','AutoSum','edit-cell'),'AutoSum'),edit('functions','Insert Function…','edit-cell'),prominent(edit('vlookup','VLOOKUP…','edit-cell'),'VLOOKUP')]),
            group('function-library','Function Library', [['financial','Financial'],['logical','Logical'],['text','Text'],['date','Date & Time'],['lookup','Lookup & Reference'],['math','Math & Trigonometry']].map(([id,label])=>prominent(edit('functions-'+id,label,'edit-cell'),({lookup:'Lookup',math:'Math'}[id]||label)))),
            group('more-functions','More',[menu('more-functions-menu','More Functions',166,[['statistical','Statistical'],['engineering','Engineering'],['information','Information'],['database','Database'],['legacy','Legacy']].map(([id,label])=>edit('functions-'+id,label,'edit-cell',false)))]),
            group('defined-names','Names',[prominent(edit('name-manager','Name Manager','create-defined-name'),'Manager'),edit('define-name','Define Name…','create-defined-name'),edit('use-name','Use in Formula…','create-defined-name')]),
            group('formula-fill','Fill',[edit('fill-down','Fill Down (Ctrl+D)','fill-range',true,()=>canFill('down')),edit('fill-right','Fill Right (Ctrl+R)','fill-range',true,()=>canFill('right'))]),
        ])],
        ['data',page('data',[
            group('sort','Sort',[edit('sort-ascending','Sort Ascending…','sort-range'),edit('sort-descending','Sort Descending…','sort-range'),prominent(edit('sort','Custom Sort…','sort-range'),'Sort…')]),
            group('data-tools','Data Tools',[prominent(edit('remove-duplicates','Remove Duplicates…','remove-duplicates'),'Remove Duplicates'),prominent(edit('text-to-columns','Text to Columns…','text-to-columns',true,canSplit),'Text to Columns')]),
            group('filter','Filter',[prominent(edit('toggle-filter','AutoFilter (Ctrl+Shift+L)','toggle-autofilter'),'Filter'),data('filter-values','Filter Values…',showFilter),prominent(data('clear-filters','Clear All Filters',clearFilters),'Clear All Filters')]),
        ])],
        ['design',page('design',[group('chart-editing','Chart',[prominent(edit('chart-type','Change Chart Type…','update-chart',true,canEditChart),'Change Chart Type…'),prominent(edit('format-chart','Format Chart…','update-chart',true,canEditChart),'Format Chart…'),prominent(edit('chart-data','Select Data…','update-chart',true,canEditChart),'Select Data…'),edit('delete-chart','Delete Chart','delete-chart',true,canEditChart)])])],
        ['view',page('view',[
            group('visibility','Show',[menu('show-options','Show',41,[decorate(toggle('toggle-formula','Formula Bar',formulaBar),'toggle-formula','Formula Bar')])]),
            group('help','Help',[menu('help-options','Help',51,[decorate(action('about','About Rice Sheet',showAbout),'about','About Rice Sheet')])]),
        ])],
    ]);
    const ribbon = new Ribbon({name:'sheet-ribbon-tabs',presentation:'office',tabs:[...pages.values()],activeTab:'home'});
    ribbon.setTabState('design',{visible:false});
    const selectTab = name => ribbon.selectTab(name);
    const file = new MenuBar({name:'sheet-file-menu',text:'panel:office-ribbon-heading:flush:fixed'},[menu('file-menu','File',null,[...(sensitivityItem ? [sensitivityItem] : []),
        doc('new','New (Ctrl+N)'),doc('open','Open… (Ctrl+O)'),doc('save','Save (Ctrl+S)'),doc('save-as','Save As…'),
        doc('save-as-xlsx','Save As XLSX…'),doc('save-as-ods','Save As ODS…'),doc('save-as-csv','Save As CSV…'),doc('save-as-tsv','Save As TSV…'),decorate(action('exit','Exit',exit),'exit','Exit'),
    ])]);
    const quickPaste = decorate(editorAction('paste', 'Paste (Ctrl+V)', 'paste-range', true, null,
        new HoldMenuButton({name:'quick-paste',text:'',icon:icons.paste,tooltip:'Paste (Ctrl+V) · Hold for options',enabled:false},[
            edit('paste-values','Paste Values','paste-range',false),
            edit('paste-formulas','Paste Formulas','paste-range',false),
        ])), 'paste', 'Paste (Ctrl+V) · Hold for options');
    const quick = new ToolBar({name:'quick-access',text:'panel:office-ribbon-heading:flush:fixed:gap=2:accessory-height=28'},[
        doc('open','Open… (Ctrl+O)',true),doc('save','Save (Ctrl+S)',true),
        edit('copy','Copy (Ctrl+C)','copy-range'),quickPaste,edit('format-painter','Format Painter','paint-format'),
        edit('undo','Undo (Ctrl+Z)','undo'),edit('redo','Redo (Ctrl+Y)','redo'),
    ]);
    for (const item of quick.children) item.text = '';

    for (const item of quick.children) item.append(new Tooltip({text:item.getProperty('tooltip')}));
    ribbon.setHeader({leading:[file],trailing:[...(sensitivityBadge ? [sensitivityBadge] : []),quick]});
    const control = ribbon;
    for(const entry of newIconControls){entry.text=entry.item.text;entry.state=entry.item.getProperty('state');}
    const refreshIcons=()=>{for(const {item,name,text,state}of newIconControls){const modern=canUseSheetDataIcons();item.setProperty('icon',modern?icons[name]:iconFallbacks[name][0]);item.text=modern?text:iconFallbacks[name][1];item.setProperty('state',modern?(item.getProperty('state')&~State.START_ALIGNED)|(state&State.START_ALIGNED):item.getProperty('state')|State.START_ALIGNED);}};
    refreshIcons();
    return Object.freeze({control,selectTab,refreshIcons,setChartSelected(selected){ribbon.setTabState('design',{visible:Boolean(selected)});if(selected)ribbon.selectTab('design');}, setNumberFormat(format) {
        const value = String(format || 'General').trim();
        // Show supported category names; retain exact custom format in tooltip.
        const label = value.toLowerCase() === 'general' ? 'General' : value.includes('%') ? 'Percentage'
            : /[$€£¥]/u.test(value) ? 'Currency' : /[dmy]/iu.test(value.replace(/"[^"]*"/gu,'')) ? 'Date'
            : /^[#0,.]+$/u.test(value) ? 'Number' : 'Custom';
        numberSelector.text = label; numberSelector.setProperty('tooltip', `Number format: ${value}`);
    }});
}

export { icons as sheetIcons };

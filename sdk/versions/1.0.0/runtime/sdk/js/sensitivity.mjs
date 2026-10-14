import { Button, Column, Dialog, Label, Row } from './controls.mjs';
import { Message } from './messages.mjs';

const guid = value => typeof value === 'string' && /^(?:[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}|\{[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\})$/iu.test(value);
const readableName = label => {
    const name = typeof label.name === 'string' ? label.name.trim() : '';
    return name && !guid(name) ? name : null;
};
export const sensitivityCaption = (labels, { compact = false } = {}) => labels?.length
    ? labels.map(label => readableName(label)
        ? `${compact ? '' : 'Sensitivity: '}${readableName(label)}`
        : readableName({name:label.documentMarking})
            ? label.documentMarking.trim() : 'Sensitivity label').join(', ') : '';
export function checkedSensitivityChange(value) {
    if (!value || Object.keys(value).some(key => !['currentId','siteId','id'].includes(key)) ||
        !guid(value.currentId) || !guid(value.siteId) || !guid(value.id)) throw new TypeError('Invalid embedded label selection');
    return { currentId:value.currentId, siteId:value.siteId, id:value.id };
}
/** Informational by default; choices come exclusively from the document. */
export function createSensitivityDialog({labels, onApply, onClose}) {
    if (!Array.isArray(labels) || !labels.length) throw new TypeError('No sensitivity label in this file');
    let pending=false;
    const content=new Column({text:'panel:clamp:620'});
    const error=new Label({name:'sensitivity-error',text:''});
    const close=new Button({name:'sensitivity-close',text:'Close'});
    const buttons=[close];
    for (const [index,label] of labels.entries()) {
        const name = readableName(label);
        content.append(new Label({name:`sensitivity-current-${index}`,text:name ? `Sensitivity: ${name}` : 'Sensitivity label applied'}));
        if (!name) content.append(new Label({name:`sensitivity-explanation-${index}`,text:'This file does not include a readable label name.'}));
        if (readableName({name:label.documentMarking})) {
            content.append(new Label({name:`sensitivity-marking-${index}`,text:`Document marking: ${label.documentMarking.trim()}`}));
            content.append(new Label({text:'The document marking may differ from the label name.'}));
        }
        content.append(new Label({text:`Label ID: ${label.id}`}));
        if (readableName({name:label.documentCompany})) content.append(new Label({name:`sensitivity-company-${index}`,text:`Document company: ${label.documentCompany.trim()}`}));
        if(label.siteId)content.append(new Label({text:`Organization ID: ${label.siteId}`}));
        if(label.choices?.length) {
            content.append(new Label({text:'Change to another label embedded in this file:'}));
            for(const [choiceIndex,choice] of label.choices.entries()) {
                const button=new Button({name:`sensitivity-choice-${index}-${choiceIndex}`,text:choice.name || choice.id});
                buttons.push(button);content.append(button);
                button.on(Message.ACTIVATE,async()=>{
                    if(pending)return;
                    pending=true;for(const b of buttons)b.setProperty('enabled',false);
                    try{await onApply(checkedSensitivityChange({currentId:label.id,siteId:label.siteId,id:choice.id}));onClose();}
                    catch(cause){error.text=String(cause.message||cause).slice(0,400);}
                    finally{pending=false;for(const b of buttons)b.setProperty('enabled',true);}
                });
            }
        }
    }
    content.append(new Label({text:labels.some(label=>label.choices?.length)
        ? 'Uses file metadata only. Organization policy is not checked.'
        : 'The file contains no usable alternative labels.'}));
    content.append(error);
    close.on(Message.ACTIVATE,()=>{if(!pending)onClose();});
    return new Dialog({name:'sensitivity-dialog',text:'Sensitivity'},[content,new Row({text:'panel:actions'},[close])]);
}

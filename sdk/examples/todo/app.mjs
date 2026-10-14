import {defineApp,createTaskWorkspace} from '@ricewm/app';
export default defineApp({async start(context){return await createTaskWorkspace(context);}});

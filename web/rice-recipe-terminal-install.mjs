/** APT package names are data; only the explicitly entered run command is shell code. */
export function validateCustomTerminals(entries = []) {
 if(!Array.isArray(entries)||entries.length>12)throw Error('A recipe supports up to 12 custom terminals');
 const seen=new Set();
 return entries.map(entry=>{
  const id=entry?.id, pkg=entry?.package, command=entry?.command||pkg;
  if(typeof id!=='string'||!/^custom-[a-z0-9-]{1,64}$/.test(id)||seen.has(id))throw Error('Invalid custom terminal identity');
  if(typeof pkg!=='string'||pkg.length>128||! /^[a-z0-9][a-z0-9+.-]+(?::[a-z0-9-]+)?$/.test(pkg))throw Error('Enter a valid APT package name');
  if(typeof command!=='string'||!command.trim()||command.length>2048||/[\0\r\n]/.test(command))throw Error('Enter a single-line run command');
  seen.add(id);return {id,package:pkg,command:command.trim()};
 });
}
export async function validateRecipePackage(packageName,execute) {
 const [{package:pkg}]=validateCustomTerminals([{id:'custom-check',package:packageName}]);
 const inspect=()=>execute(`apt-cache show -- '${pkg}'`);
 let result=await inspect();
 const exists=value=>value['exit code']===0 && String(value.stdout||'').split(/\r?\n/).some(line=>line.trim()==='Package: '+pkg.split(':')[0]);
 if(!exists(result)){const update=await execute('apt-get update');if(update['exit code']!==0)throw Error('APT catalog could not be refreshed. Try again when the connection is available.');result=await inspect();}
 if(!exists(result))throw Error(`APT package “${pkg}” was not found in the configured repositories.`);
 return {package:pkg,valid:true};
}
export async function installRecipeTools(tools, installPackage, installBinary, {custom=[],onProgress=()=>{}}={}) {
 const entries=validateCustomTerminals(custom),byId=new Map(entries.map(entry=>[entry.id,entry]));
 if(!Array.isArray(tools)||!tools.length||tools.length>18||tools.some(id=>!['fastfetch','btop','htop','cava','yazi','mc'].includes(id)&&!byId.has(id)))throw Error('Invalid recipe tools');
 const ids=[...new Set(tools)],results=[];
 onProgress({completed:0,total:ids.length,message:'Preparing installation…'});
 let completed=0;
 for(const id of ids){
  const entry=byId.get(id),pkg=entry?.package||id;
  onProgress({completed,total:ids.length,message:`Installing ${pkg}…`});
  let result;
  try{
   result=id==='yazi'&&installBinary?await installBinary(id):await installPackage(pkg);
   if(id==='cava'&&result['exit code']===0)result=await installPackage('bsdutils');
  }catch(error){result={'exit code':1,stderr:error.message};}
  results.push({id,installed:result['exit code']===0,error:result['exit code']===0?'':(result.stderr||result.stdout||'Package installation failed').slice(-1800)});
  onProgress({completed:++completed,total:ids.length,message:result['exit code']===0?`${pkg} installed`:`${pkg} installation failed`});
 }
 return {results};
}

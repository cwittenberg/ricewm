import {EFFECT_LIMITS} from './rice-effect-contract.mjs';
// Pinned parsers are loaded only when a user opens/qualifies authored source.
let parsers;
const load=()=>parsers??=import('./vendor/rice-shader-parsers.mjs');
const PURE=new Set(('abs acos acosh asin asinh atan atan2 atanh ceil clamp cos cosh cross degrees distance dot exp exp2 floor fract inverse inverseSqrt inversesqrt length log log2 max min mix normalize pow radians reflect refract round sign sin sinh smoothstep sqrt step tan tanh transpose trunc mod fma select saturate riceInput riceSignals vec2 vec3 vec4 vec2f vec3f vec4f vec2i vec3i vec4i vec2u vec3u vec4u mat2 mat3 mat4 mat2x2f mat3x3f mat4x4f float int uint bool f32 i32 u32').split(' '));
const kind=n=>n?.astNodeType||n?.type;
const name=n=>n?.identifier||n?.name||n?.specifier?.token||n?.specifier?.specifier?.token;
const children=n=>Array.isArray(n)?n:n&&typeof n==='object'?Object.entries(n).filter(([k])=>!['location','scopes','parent','calls','typeInfo','data','attributes'].includes(k)).map(([,v])=>v).filter(v=>v&&typeof v==='object'):[];
function each(n,visitor,depth=0){if(depth>100)throw Error('Shader syntax is too deeply nested');if(n&&typeof n==='object'){visitor(n);for(const c of children(n))each(c,visitor,depth+1);}}
const fail=(message,node)=>{const line=node?.location?.start?.line||node?.line;throw Object.assign(Error((line?`Line ${line}: `:'')+message),{line});};
function lexicalBudget(source,language){
 if(typeof source!=='string'||!source.trim()||new TextEncoder().encode(source).length>EFFECT_LIMITS.sourceBytes)throw Error('Shader source must be between 1 byte and 64 KiB');
 // This is a parser resource guard, not a language validator. The AST below
 // decides executable semantics; preprocessing never runs on user source.
 const clean=source.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g,' '),tokens=clean.match(/[A-Za-z_]\w*|\d+(?:\.\d*)?|[^\s]/g)||[];
 if(tokens.length>12000)throw Error('Shader has too many tokens');
 let depth=0;for(const t of tokens){if(['{','(','['].includes(t)){if(++depth>64)throw Error('Shader syntax is too deeply nested');}else if(['}',')',']'].includes(t))depth--;}
 if(tokens.includes('#'))throw Error('Shader preprocessor directives are not supported');
 if(language==='wgsl'&&tokens.some(t=>['const','override','alias','enable','diagnostic'].includes(t)))throw Error('Use functions and local let/var declarations in this shader profile');
 return clean;
}
function loopInfo(n,language){
 if(language==='glsl'){
  const d=n.init?.declarations?.[0],variable=name(d?.identifier),condition=n.condition,operation=n.operation;
  const limit=condition?.right?.type==='int_constant'?Number(condition.right.token):NaN;
  if(n.init?.declarations?.length!==1||name(n.init.specified_type?.specifier)!=='int'||d.initializer?.type!=='int_constant'||Number(d.initializer.token)!==0||d.quantifier||condition?.type!=='binary'||condition.operator?.literal!=='<'||name(condition.left)!==variable||!Number.isInteger(limit)||limit<1||limit>EFFECT_LIMITS.iterations||!((operation?.type==='postfix'&&name(operation.expression)===variable&&operation.postfix?.literal==='++')||(operation?.type==='unary'&&name(operation.expression)===variable&&operation.operator?.literal==='++')))fail('Use a fixed loop: for (int i=0; i<N; i++), with N between 1 and 64',n);
  return {variable,limit,body:n.body};
 }
 const variable=n.init?.name,condition=n.condition,limit=condition?.right?.value?.data?.[0];
 if(kind(n.init)!=='var'||n.init.type?.name!=='i32'||n.init.value?.value?.data?.[0]!==0||kind(condition)!=='binaryOp'||condition.operator!=='<'||kind(condition.left)!=='varExpr'||condition.left.name!==variable||!Number.isInteger(limit)||limit<1||limit>EFFECT_LIMITS.iterations||kind(n.increment)!=='increment'||n.increment.operator!=='++'||n.increment.variable?.name!==variable)fail('Use a fixed loop: for (var i:i32=0; i<N; i++), with N between 1 and 64',n);
 return {variable,limit,body:n.body};
}
function assertCounterUnchanged(body,variable,language){
 each(body,n=>{
  const k=kind(n);
  const target=language==='glsl'?n.left||n.expression:n.variable;
  if(['assignment','assign','increment','postfix','unary'].includes(k)&&name(target)===variable&&(k!=='unary'||['++','--'].includes(n.operator?.literal)))fail('A fixed-loop counter cannot be modified inside its body',n);
  if((language==='glsl'&&k==='declaration'&&name(n.identifier)===variable)||(language==='wgsl'&&['var','let'].includes(k)&&n.name===variable))fail('A fixed-loop counter cannot be shadowed',n);
 });
}
export async function qualifyRiceShader(source,language){
 return qualifyRiceShaderWithParsers(source,language,await load());
}
export function qualifyRiceShaderWithParsers(source,language,{parseGlsl,WgslParser}){
 if(!['glsl','wgsl'].includes(language))throw Error('Unsupported shader language');
 lexicalBudget(source,language);
 let ast;try{ast=language==='glsl'?parseGlsl(source,{quiet:true,includeLocation:true,stage:'fragment'}).program:new WgslParser().parse(source);}catch(error){throw Error(`Invalid ${language.toUpperCase()}: ${error.message}`);}
 if(!ast.length||ast.length>EFFECT_LIMITS.functions||ast.some(n=>kind(n)!=='function'))throw Error('Shader source may declare functions only; RiceWM owns all resources and entry points');
 const functions=new Map();
 for(const fn of ast){
  const id=language==='glsl'?name(fn.prototype.header.name):fn.name;
  if(!id||functions.has(id)||PURE.has(id)||id.startsWith('gl_')||(id.startsWith('rice')&&!['riceEffect','riceMaterial','riceDisplace'].includes(id)))fail('Duplicate or reserved shader function name',fn);
  if(language==='wgsl'&&(fn.attributes?.length||fn.args.some(a=>a.attributes?.length)))fail('Shader entry point attributes belong to RiceWM',fn);
  if(language==='glsl'&&fn.prototype.parameters?.some(p=>p.qualifier?.some(q=>['out','inout'].includes(q.token))))fail('Shader helpers must use value parameters',fn);
  functions.set(id,fn);
 }
 if(!functions.has('riceEffect')&&!functions.has('riceMaterial'))throw Error('Define riceEffect(uv, time, params), or riceMaterial(position, normal, uv, time, params)');
 const measured=new Map();
 function costFunction(id,stack=[]){
  if(stack.includes(id))fail('Recursive shader helpers are not supported',functions.get(id));
  if(measured.has(id))return measured.get(id);
  const fn=functions.get(id);if(!fn)return 1;
  const cost=costNode(fn.body,[...stack,id]);measured.set(id,cost);return cost;
 }
 function costNode(n,stack){
  if(!n||typeof n!=='object')return 0;
  const k=kind(n);
  if(['while_statement','do_statement','while','loop','continuing','discard','discard_statement','struct','pointer','arrayType','atomic'].includes(k))fail('Unbounded loops, discard, arrays, pointers and storage are outside this shader profile',n);
  if(language==='wgsl'&&k==='unaryOp'&&['&','*'].includes(n.operator))fail('Shader pointers and address aliases are outside this profile',n);
  if(n.quantifier||k==='type'&&/^(ptr|array|atomic|texture|sampler)/.test(n.name||''))fail('Shader resources and arrays belong to RiceWM',n);
  if(language==='glsl'&&k==='keyword'&&['uniform','buffer','shared','layout','sampler2D','inout','out'].includes(n.token))fail('Shader resources belong to RiceWM',n);
  if(language==='wgsl'&&(n.storage||n.access||n.attributes?.length))fail('Shader bindings and address spaces belong to RiceWM',n);
  if(k==='for_statement'||k==='for'){
   const {variable,limit,body}=loopInfo(n,language);assertCounterUnchanged(body,variable,language);
   const cost=10+limit*costNode(body,stack);if(cost>EFFECT_LIMITS.work)fail('Shader exceeds the bounded work budget (8192 operations per pixel)',n);return cost;
  }
  let cost=1;
  if(k==='function_call'||k==='callExpr'||k==='call'){
   const id=language==='glsl'?name(n.identifier):n.name;
   if(!PURE.has(id)&&!functions.has(id))fail(`Unsupported shader helper: ${id}`,n);
   cost+=functions.has(id)?costFunction(id,stack):4;
  }
  for(const c of children(n))cost+=costNode(c,stack);
  if(cost>EFFECT_LIMITS.work)fail('Shader exceeds the bounded work budget (8192 operations per pixel)',n);
  return cost;
 }
 for(const id of functions.keys())costFunction(id);
 return {language,work:Math.max(...measured.values()),functions:[...functions.keys()],profile:'rice.bounded-fragment/v1'};
}
export async function qualifyRiceEffect(effect){
 const result={};for(const [language,source] of Object.entries(effect.implementations))result[language]=await qualifyRiceShader(source,language);return result;
}

import {readFileSync,appendFileSync} from 'node:fs';
import {resolve,relative} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
export function validateInput(input, allowed) {
  const repos=JSON.parse(allowed.repositories || '[]');
  const prefixes=JSON.parse(allowed.packagePrefixes || '[]');
  if(!Array.isArray(repos)||!Array.isArray(prefixes)||!prefixes.length)throw Error('Configure repository and package-prefix allowlists');
  for(const repo of [input.source,input.destination])if(!/^[A-Za-z0-9-]+\/[A-Za-z0-9_.-]+$/.test(repo)||!repos.includes(repo))throw Error('Repository is not allowlisted');
  if(!/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(input.tag)||input.tag.includes('..'))throw Error('Source must be an explicit immutable tag');
  if(!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(input.version))throw Error('Pin an exact Kaji version');
  if(!['npm','pypi','crates.io','go'].includes(input.registry))throw Error('Unsupported test registry');
  if(!prefixes.includes(input.prefix)||input.prefix.length<4)throw Error('Test package prefix is not allowlisted');
  if(!['preview','open_pr'].includes(input.mode))throw Error('Unknown mode');
  if(!/^[a-z][a-z0-9]*$/.test(input.language))throw Error('Invalid language');
  for(const path of [input.config])if(!path||path.startsWith('/')||path.split(/[\\/]/).some(p=>p==='..'||p==='.git')||/[\x00-\x1f]/.test(path))throw Error('Unsafe recipe path');
  return {...input,publication:false,merge:false};
}
export function validateRecipe(recipe,input) {
  const packages=recipe.packages?.filter(p=>p.language===input.language);
  if(!packages?.length)throw Error('No selected SDK packages');
  for(const p of packages)if(typeof p.name!=='string'||!p.name.startsWith(input.prefix)||p.release?.publisher?.registry!==input.registry)throw Error('Every selected package must use the allowlisted test prefix and registry');
  const root=recipe.output?.path;
  if(typeof root!=='string'||!root||root.startsWith('/')||root.split(/[\\/]/).some(p=>p==='..'||p==='.git'))throw Error('Unsafe output root');
  return {root,packages:packages.map(p=>({path:p.path,language:p.language,name:p.name}))};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const input={source:process.env.SOURCE_REPOSITORY,destination:process.env.DESTINATION_REPOSITORY,tag:process.env.SOURCE_TAG,version:process.env.KAJI_VERSION,registry:process.env.TEST_REGISTRY,prefix:process.env.TEST_PACKAGE_PREFIX,mode:process.env.DELIVERY_MODE,language:process.env.SDK_LANGUAGE,config:process.env.SDK_CONFIG};
  const plan=validateInput(input,{repositories:process.env.ALLOWED_REPOSITORIES,packagePrefixes:process.env.ALLOWED_PACKAGE_PREFIXES});
  if(process.argv.includes('--recipe')) {
    const recipe=validateRecipe(JSON.parse(readFileSync(resolve('probe',plan.config),'utf8')),plan);
    const root=resolve('probe',plan.config,'..',recipe.root);
    if(relative(resolve('probe'),root).startsWith('..'))throw Error('Output escapes disposable checkout');
    if(!process.argv.includes('--check-only')) for(const p of recipe.packages) {
      const packageRoot=resolve(root,p.path);
      if(relative(root,packageRoot).startsWith('..'))throw Error('Package path escapes output');
      const metadata=JSON.parse(readFileSync(resolve(packageRoot,'.kaji/package.json'),'utf8'));
      if(metadata.language!==plan.language||typeof metadata.name!=='string'||!metadata.name.startsWith(plan.prefix)||metadata.publisher?.registry!==plan.registry)throw Error('Emitted metadata is outside disposable publication scope');
    }
    if(process.env.GITHUB_OUTPUT)appendFileSync(process.env.GITHUB_OUTPUT,`root=${root}\n`);
    if(!process.argv.includes('--check-only')) for(const p of recipe.packages)for(const phase of ['build','test']) {
      const result=spawnSync('kaji',['sdk','run','--root',root,'--package',p.path,'--phase',phase],{cwd:resolve('probe'),stdio:'inherit'});
      if(result.status!==0)throw Error('Native SDK check failed');
    }
  }
  console.log(JSON.stringify(plan,null,2));
}

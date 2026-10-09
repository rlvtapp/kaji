import assert from 'node:assert/strict';
import React from 'react';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';

import {effectScope,ref} from 'vue';
import {QueryClient,QueryClientProvider,MutationObserver} from '@tanstack/react-query';
import {QueryClient as VueQueryClient} from '@tanstack/vue-query';
globalThis.window={addEventListener(){},removeEventListener(){}};
const {SWRConfig}=await import('swr');
const {reactQuery,vueQuery,swr,createGraphqlHttpTransport}=await import('./dist/index.js');
let calls=0;let seenSignal;
const transport={async execute(document,name,variables,options){calls++;seenSignal=options?.signal;return variables.id==='partial'?{kind:'partial',data:{read:'partial'},errors:[{message:'visible'}]}:variables.id==='error'?{kind:'error',errors:[{message:'visible'}]}:{kind:'success',data:name==='Rename'?{rename:variables.name}:{read:variables.id}};}};
const client=new QueryClient({defaultOptions:{queries:{retry:false,staleTime:Infinity}}});
const a=reactQuery.readQueryOptions(transport,'endpoint/user',{id:'one'});
assert.equal((await client.fetchQuery(a)).data.read,'one');await client.fetchQuery(a);assert.equal(calls,1);
assert.ok(seenSignal instanceof AbortSignal);
assert.equal((await client.fetchQuery(reactQuery.readQueryOptions(transport,'endpoint/user',{id:'two'}))).data.read,'two');
assert.equal((await client.fetchQuery(reactQuery.readQueryOptions(transport,'other/user',{id:'one'}))).data.read,'one');
const other={execute:transport.execute};await client.fetchQuery(reactQuery.readQueryOptions(other,'endpoint/user',{id:'one'}));assert.equal(calls,4);
assert.equal((await client.fetchQuery(reactQuery.readQueryOptions(transport,'endpoint/user',{id:'partial'}))).kind,'partial');
assert.equal((await client.fetchQuery(reactQuery.readQueryOptions(transport,'endpoint/user',{id:'error'}))).kind,'error');
const vue=new VueQueryClient({defaultOptions:{queries:{retry:false}}});assert.equal((await vue.fetchQuery(vueQuery.readQueryOptions(transport,'endpoint/user',{id:'vue'}))).data.read,'vue');
assert.deepEqual(reactQuery.readQueryOptions(transport,'endpoint/user',{id:'one'}).queryKey,vueQuery.readQueryOptions(transport,'endpoint/user',{id:'one'}).queryKey);
assert.deepEqual(swr.readQueryOptions(transport,'endpoint/user',{id:'one'}).key,a.queryKey);
assert.equal((await swr.readQueryOptions(transport,'endpoint/user',{id:'swr'}).fetcher()).data.read,'swr');
assert.equal((await swr.renameMutationOptions(transport,'endpoint/user').fetcher(null,{arg:{name:'SWR'}})).data.rename,'SWR');
const mutation=reactQuery.renameMutationOptions(transport,'endpoint/user');assert.equal(mutation.retry,false);assert.equal((await mutation.mutationFn({name:'Ada'})).data.rename,'Ada');
let aborted=false;const pending={execute:(_d,_n,_v,options)=>new Promise((_,reject)=>options.signal.addEventListener('abort',()=>{aborted=true;reject(new DOMException('aborted','AbortError'));}))};
const options=reactQuery.readQueryOptions(pending,'endpoint/user',{id:'abort'});const promise=client.fetchQuery(options).catch(error=>error);await new Promise(resolve=>setTimeout(resolve,10));await client.cancelQueries({queryKey:options.queryKey});await promise;assert.equal(aborted,true);
let failedMutations=0;const failing={execute:async()=>{failedMutations++;throw new Error('network');}};const observer=new MutationObserver(client,reactQuery.renameMutationOptions(failing,'endpoint/user'));await observer.mutate({name:'failure'}).catch(()=>{});assert.equal(failedMutations,1);
client.clear();vue.clear();

const scope=effectScope();let vueHook;let vueMutation;const reactiveVariables=ref({id:'vue-hook'});const reactiveScope=ref('mounted/vue');
scope.run(()=>{vueHook=vueQuery.useRead(transport,reactiveScope,()=>reactiveVariables.value, {}, vue);vueMutation=vueQuery.useRename(transport,'mounted/vue',{},vue);});
await new Promise(resolve=>setTimeout(resolve,20));assert.equal(vueHook.data.value.data.read,'vue-hook');
reactiveVariables.value.id='vue-next';for(let i=0;i<30&&vueHook.data.value?.data.read!=='vue-next';i++)await new Promise(resolve=>setTimeout(resolve,10));assert.equal(vueHook.data.value.data.read,'vue-next');assert.equal(vue.getQueryData(vueQuery.readQueryOptions(transport,'mounted/vue',{id:'vue-hook'}).queryKey).data.read,'vue-hook');
const beforeScopeChange=calls;reactiveScope.value='mounted/vue-other';for(let i=0;i<30&&calls===beforeScopeChange;i++)await new Promise(resolve=>setTimeout(resolve,10));assert.ok(calls>beforeScopeChange);assert.equal((await vueMutation.mutateAsync({name:'vue-mutation'})).data.rename,'vue-mutation');scope.stop();
if(process.env.POOLSTER_REACT_TEST_RENDERER){
 const require=createRequire(import.meta.url);const {create,act}=require(process.env.POOLSTER_REACT_TEST_RENDERER);globalThis.IS_REACT_ACT_ENVIRONMENT=true;
 let reactHook;let swrHook;let swrMutation;
 function ReactProbe(){reactHook=reactQuery.useRead(transport,'mounted/react',{id:'react-hook'});return null;}
 function SwrProbe(){swrHook=swr.useRead(transport,'mounted/swr',{id:'swr-hook'});swrMutation=swr.useRename(transport,'mounted/swr');void swrHook.data;return null;}
 let rendered;
 await act(async()=>{rendered=create(React.createElement(QueryClientProvider,{client},React.createElement(ReactProbe)));});
 await act(async()=>{await new Promise(resolve=>setTimeout(resolve,20));});assert.equal(reactHook.data.data.read,'react-hook');
 await act(async()=>{rendered.unmount();});
 await act(async()=>{rendered=create(React.createElement(SWRConfig,{value:{provider:()=>new Map(),dedupingInterval:0}},React.createElement(SwrProbe)));});
 await act(async()=>{await new Promise(resolve=>setTimeout(resolve,20));});assert.equal(swrHook.data.data.read,'swr-hook');
 let renamed;await act(async()=>{renamed=await swrMutation.trigger({name:'mounted-rename'});});assert.equal(renamed.data.rename,'mounted-rename');
 await act(async()=>{rendered.unmount();});
}
client.clear();vue.clear();

const protocolRequire=createRequire(import.meta.url);const {buildSchema,graphql}=protocolRequire(process.env.POOLSTER_GRAPHQL_JS);
const schema=buildSchema('type Query {read(id:ID!):String!} type Mutation {rename(name:String!):String!}');
const server=createServer(async(request,response)=>{let body='';for await(const chunk of request)body+=chunk;const input=JSON.parse(body);const result=await graphql({schema,source:input.query,operationName:input.operationName,variableValues:input.variables,rootValue:{read:({id})=>'server-'+id,rename:({name})=>name}});response.setHeader('content-type','application/json');response.end(JSON.stringify(result));});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
try {
 const endpoint=`http://127.0.0.1:${server.address().port}`;const httpTransport=createGraphqlHttpTransport(endpoint);
 assert.equal((await client.fetchQuery(reactQuery.readQueryOptions(httpTransport,endpoint,{id:'http'}))).data.read,'server-http');
 assert.equal((await vue.fetchQuery(vueQuery.readQueryOptions(httpTransport,endpoint,{id:'vue-http'}))).data.read,'server-vue-http');
 assert.equal((await swr.readQueryOptions(httpTransport,endpoint,{id:'swr-http'}).fetcher()).data.read,'server-swr-http');
 assert.equal((await reactQuery.renameMutationOptions(httpTransport,endpoint).mutationFn({name:'server-mutation'})).data.rename,'server-mutation');
 const {create,act}=protocolRequire(process.env.POOLSTER_REACT_TEST_RENDERER);let observed;
 function HttpProbe(){observed=swr.useRead(httpTransport,endpoint,{id:'mounted-http'});void observed.data;return null;}
 let rendered;await act(async()=>{rendered=create(React.createElement(SWRConfig,{value:{provider:()=>new Map(),dedupingInterval:0}},React.createElement(HttpProbe)));});
 for(let i=0;i<30&&!observed.data;i++)await act(async()=>{await new Promise(resolve=>setTimeout(resolve,10));});
 assert.equal(observed.data.data.read,'server-mounted-http');await act(async()=>{rendered.unmount();});
} finally {client.clear();vue.clear();await new Promise(resolve=>server.close(resolve));}

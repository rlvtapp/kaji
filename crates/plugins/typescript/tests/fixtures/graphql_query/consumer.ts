import {reactQuery,vueQuery,swr,createGraphqlHttpTransport} from './index.js';
const transport=createGraphqlHttpTransport('http://localhost/graphql');
reactQuery.readQueryOptions(transport,'endpoint/user',{id:'7'});
vueQuery.readQueryOptions(transport,'endpoint/user',{id:'7'});
swr.readQueryOptions(transport,'endpoint/user',{id:'7'});
reactQuery.renameMutationOptions(transport,'endpoint/user').mutationFn({name:'Ada'});
// @ts-expect-error selection-specific variables remain required
reactQuery.readQueryOptions(transport,'endpoint/user',{});
// @ts-expect-error no undeclared variables
swr.readQueryOptions(transport,'endpoint/user',{id:'7',other:true});
async function selected(){const response=await reactQuery.readQueryOptions(transport,'endpoint/user',{id:'7'}).queryFn!({signal:new AbortController().signal} as never);if(response.kind!=='error'){
 response.data.read;
 // @ts-expect-error unselected output field
 response.data.other;
}}
void selected;

import {ref} from 'vue';
const variables=ref({id:'reactive'});
vueQuery.useRead(transport,ref('endpoint/user'),variables);
vueQuery.useRead(transport,()=> 'endpoint/user',()=>variables.value);
// @ts-expect-error reactive variables must match the selected operation
vueQuery.useRead(transport,'endpoint/user',ref({id:7}));

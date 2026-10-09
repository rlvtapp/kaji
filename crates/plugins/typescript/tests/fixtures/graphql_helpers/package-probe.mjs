import assert from 'node:assert/strict';
import { setupServer } from 'msw/node';
import { QueryClient } from '@tanstack/react-query';
import { createClient, createGraphqlHttpTransport, zod, faker, msw, reactQuery, vueQuery, swr } from 'poolster-graphql-ecosystem-test';
const server=setupServer(msw.mockReadUser(({id})=>({data:{user:{id,name:'Ada',nickname:null}},...(id==='partial'?{errors:[{message:'visible'}]}:{})})),msw.mockRename(({name})=>({data:{rename:{id:'42',name,nickname:null}}})));
server.listen({onUnhandledRequest:'error'});
const queryClient=new QueryClient({defaultOptions:{queries:{retry:false}}});
try {
 const transport=createGraphqlHttpTransport('http://example.test/graphql');
 const client=createClient({transport});
 assert.equal((await client.readUser({id:'42'})).data.user.name,'Ada');
 assert.equal((await client.rename({name:'Grace'})).data.rename.name,'Grace');
 assert.equal((await client.readUser({id:'partial'})).kind,'partial');
 assert.equal(zod.ReadUserResultSchema.safeParse(faker.fakeReadUserResult()).success,true);
 assert.equal(zod.ReadUserResultSchema.safeParse({user:{id:'42',name:'Ada'}}).success,false);
 assert.equal((await queryClient.fetchQuery(reactQuery.readUserQueryOptions(transport,'public',{id:'42'}))).data.user.name,'Ada');
 assert.equal((await queryClient.fetchQuery(vueQuery.readUserQueryOptions(transport,'public',{id:'vue'}))).data.user.id,'vue');
 assert.equal((await swr.readUserQueryOptions(transport,'public',{id:'swr'}).fetcher()).data.user.id,'swr');
 console.log('Installed tarball: flat client, query integrations, Zod, Faker and MSW passed');
}finally{queryClient.clear();server.close();}

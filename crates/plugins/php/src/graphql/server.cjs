const {graphql,buildSchema,version}=require(process.env.POOLSTER_GRAPHQL_JS_ROOT+'/graphql');
if(version!=='16.14.2')throw Error('Expected pinned GraphQL16.14.2, got '+version);
const schema=buildSchema('type Query { readUser(id: ID!): User } type User { name: String! nickname: String }');
const server=require('node:http').createServer(async(req,res)=>{
let body='';for await(const chunk of req)body+=chunk;
const {query,variables,operationName}=JSON.parse(body);
const result=await graphql({schema,source:query,variableValues:variables,operationName,rootValue:{readUser:({id})=>{if(id==='error')throw Error('denied');return{name:'Ada',nickname:null}}}});
res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify(result));
});server.listen(0,'0.0.0.0',()=>console.log(server.address().port));

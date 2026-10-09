const root=process.env.POOLSTER_GRAPHQL_JS_ROOT;
const g=require(root+'/graphql');
if(require(root+'/graphql/package.json').version!=='16.14.2')throw Error('GraphQL.js pin');
const schema=g.buildSchema('type Query {readUser(id: ID!,filter: Filter):User nodes:[Node]! matrix:[[String]]} type User {name:String! nickname:String} type Mutation {ping:Boolean!} input Filter {name:String} type Team {name:String!} union Node = User | Team');
const server=require('http').createServer(async(req,res)=>{let body='';for await(const c of req)body+=c;const v=JSON.parse(body);const result=await g.graphql({schema,source:v.query,operationName:v.operationName,variableValues:v.variables,rootValue:{nodes:()=>[{__typename:'User',name:'Ada'},null,{__typename:'Team',name:'Staff'}],matrix:()=>[['a',null],null],ping:()=>true,readUser:({id})=>({name:'Ada',nickname:()=>{if(id==='partial')throw Error('nickname failed');return null}})}});res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));});
server.listen(0,'0.0.0.0',()=>console.log(server.address().port));

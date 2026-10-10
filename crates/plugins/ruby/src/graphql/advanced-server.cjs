
const {createHandler}=require(process.env.POOLSTER_GRAPHQL_SSE_ROOT+'/graphql-sse/lib/use/http');
const {buildSchema,graphql}=require(process.env.POOLSTER_GRAPHQL_SSE_ROOT+'/graphql');
const schema=buildSchema('scalar DateTime type Query {hello(id:DateTime):DateTime} type Subscription {hello(id:DateTime):DateTime}');
const date=schema.getType('DateTime');date.parseValue=v=>new Date(v);date.serialize=v=>v.toISOString();
schema.getSubscriptionType().getFields().hello.subscribe=async function*(_source,args){yield{hello:args.id};};
const handler=createHandler({schema});require('http').createServer(async(req,res)=>{
if(req.headers.authorization!=='Bearer secret'){res.statusCode=401;res.end();return;}
if(req.url==='/multipart'){res.setHeader('Content-Type','multipart/mixed; boundary=parts; deferSpec=20220824');for(const frame of[{data:{},hasNext:true},{incremental:[{path:[],data:{hello:'2025-01-01T00:00:00.000Z'}}],hasNext:false}])res.write('--parts\r\nContent-Type: application/json\r\n\r\n'+JSON.stringify(frame)+'\r\n');res.end('--parts--\r\n');return;}
if(req.headers.accept==='text/event-stream'){await handler(req,res);return;}let body='';for await(const chunk of req)body+=chunk;const v=JSON.parse(body);res.setHeader('Content-Type','application/json');res.end(JSON.stringify(await graphql({schema,source:v.query,operationName:v.operationName,variableValues:v.variables,rootValue:{hello:a=>a.id}})));
}).listen(0,'0.0.0.0',function(){console.log(this.address().port)});

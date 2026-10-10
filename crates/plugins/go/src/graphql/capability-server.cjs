const http=require('http'),root=process.env.POOLSTER_GRAPHQL_SSE_ROOT;
if(require(root+'/graphql-sse/package.json').version!=='2.6.0'||require(root+'/graphql/package.json').version!=='16.14.2')throw Error('expected pinned GraphQL.js16.14.2/graphql-sse2.6.0');
const {buildSchema,graphql}=require(root+'/graphql');const{createHandler}=require(root+'/graphql-sse/lib/use/http');
const schema=buildSchema(require('fs').readFileSync(process.argv[2],'utf8'));
const date=schema.getType('DateTime');date.parseValue=value=>{if(typeof value!=='string'||!/^wire-\d+$/.test(value))throw Error('invalid input wire scalar');return Number(value.slice(5));};date.serialize=value=>{if(!Number.isInteger(value))throw Error('invalid output wire scalar');return 'wire-'+value;};
let closed=0;schema.getSubscriptionType().getFields().ticks.subscribe=async function*(_source,args){try{yield{ticks:{when:args.start,maybe:null,list:[args.start,null],name:'é'}};if(args.start===99){while(true){await new Promise(resolve=>setTimeout(resolve,30));yield{ticks:{when:args.start,maybe:null,list:null,name:'wait'}};}}yield{ticks:{when:args.start,maybe:'invalid',list:null,name:'second'}};}finally{closed++;}};
const sse=createHandler({schema});
const server=http.createServer(async(req,res)=>{
 if(req.url==='/closed'){res.setHeader('content-type','application/json');res.end(JSON.stringify({closed}));return;}
 if(req.headers.authorization!=='Bearer secret'){res.writeHead(401);res.end('unauthorized');return;}
 if(req.url.startsWith('/sse-')){
   res.setHeader('content-type','text/event-stream');const body=req.url==='/sse-split'?': heartbeat\r\nevent: next\r\ndata: {"data":\r\ndata: {"ticks":{"when":"wire-7","maybe":null,"list":null,"name":"é"}}}\r\n\r\nevent: complete\r\ndata: \r\n\r\n':req.url==='/sse-eof'?'event: next\ndata: {"data":{"ticks":{"when":"wire-7","maybe":null,"list":null,"name":"first"}}}\n\n':'event: unknown\ndata: {}\n\n';
   for(const byte of Buffer.from(body)){res.write(Buffer.from([byte]));}res.end();return;
 }
 if(req.headers.accept==='text/event-stream'){await sse(req,res);return;}
 let body='';for await(const chunk of req)body+=chunk;const payload=JSON.parse(body);
 if(req.headers.accept.includes('multipart/mixed')){
   const final={data:{user:{id:'7',when:'wire-7'},values:[1,2,3]}};
   if(req.url==='/plain'){res.setHeader('content-type','application/json');res.end(JSON.stringify(final));return;}
   res.setHeader('content-type','multipart/mixed; boundary="poolster"; deferSpec=20220824');
   const frames=[{data:{user:{id:'7'},values:[1]},hasNext:true},{incremental:[{path:req.url==='/invalid-path'?['wrong']:['user'],label:'details',data:{when:'wire-7'}}],hasNext:true},{incremental:[{path:['values',1],label:'values',items:[2,3]}],hasNext:false}];
   if(req.url==='/dialect')frames[1]={pending:[],hasNext:true};
   let output=frames.map(frame=>'--poolster\r\nContent-Type: application/json\r\n\r\n'+JSON.stringify(frame)+'\r\n').join('');if(req.url!=='/eof')output+='--poolster--\r\n';
   for(const byte of Buffer.from(output)){res.write(Buffer.from([byte]));}res.end();return;
 }
 const result=await graphql({schema,source:payload.query,operationName:payload.operationName,variableValues:payload.variables,rootValue:{stamp:({input})=>{if(input.when!==7||input.next.when!==8||input.list[0]!==null||input.list[1]!==9)throw Error('nested input codecs not applied');return{when:input.when,maybe:null,list:input.list,name:'read'};}}});res.setHeader('content-type','application/json');res.end(JSON.stringify(result));
});server.listen(0,'127.0.0.1',()=>console.log('http://127.0.0.1:'+server.address().port));

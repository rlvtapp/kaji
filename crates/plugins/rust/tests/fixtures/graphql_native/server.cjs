const http = require('node:http');
const {buildSchema, graphql} = require(process.env.POOLSTER_GRAPHQL_JS);
const schema = buildSchema(require('node:fs').readFileSync(process.argv[2], 'utf8'));
const timestamp = schema.getType('Timestamp');
timestamp.parseValue = value => {if(typeof value !== 'string')throw new Error('Timestamp input must be string');return value;};
timestamp.serialize = value => {if(!Number.isSafeInteger(value))throw new Error('Timestamp output must be integer');return value;};
const root = {
 scalars: ({value,input,optional}) => {
  if(value!=='wire-time'||input.required!=='nested-time'||input.values[0]!=='list-time'||input.values[1]!==null)throw new Error('scalar input wire mismatch');
  if(input.optional!==null||optional!==undefined)throw new Error('scalar null/omission mismatch');
  return {timestamp:1700000000,values:[1700000001,null],nullable:null,optional:1700000002,raw:{retained:['json',42]}};
 },
 node: ()=>({__typename:'Robot',id:'r1',code:'retained-second-variant'}),
 user: ({id}) => ({id, name:'Ada', nickname:null, fragile:()=>{throw new Error('field failed')}}),
 rename: ({name}) => ({id:'7',name,nickname:null}),
 fatal: ()=>{throw new Error('root failed')},
 echo: ({input})=>input,
 inputEcho: ({options})=>options===undefined?'options-omitted':options===null?'options-null':options.note,
};
const server = http.createServer(async(req,res)=>{
 try {
 if(req.url==='/style'&&req.headers['x-style']!=='configured-once')throw new Error('client headers missing');
 if(req.url==='/bad-scalar'){res.end(JSON.stringify({data:{scalars:{timestamp:'not-an-integer',values:[1700000001,null],nullable:null,raw:{retained:['json',42]}}}}));return;}
 if(req.url==='/disconnect'){req.socket.destroy();return;}
 if(req.url==='/http-error'){res.writeHead(503);res.end('unavailable');return;}
 if(req.url==='/bad-json'){res.end('{');return;}
 if(req.url==='/bad-envelope'){res.end('{"data":123}');return;}
 if(req.url==='/missing-nullable'){res.end('{"data":{"person":{"id":"7","name":"Ada"}}}');return;}
 if(req.url==='/null-nonnull'){res.end('{"data":{"user":{"name":null,"nickname":null}}}');return;}
 let body='';for await(const chunk of req)body+=chunk;
 if(req.url==='/slow')await new Promise(resolve=>setTimeout(resolve,100));
 const request=JSON.parse(body);
 const result=await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:root});
 res.setHeader('content-type','application/json');res.end(JSON.stringify(result));
 } catch(error) {
  if(error.code === 'ECONNRESET' && req.destroyed) return;
  console.error(error);res.writeHead(500);res.end('fixture error');
 }
}).on('clientError', (error, socket) => {
 if(error.code !== 'ECONNRESET') console.error(error);
 socket.destroy();
});
server.listen(0,'127.0.0.1',()=>console.log(`http://127.0.0.1:${server.address().port}`));

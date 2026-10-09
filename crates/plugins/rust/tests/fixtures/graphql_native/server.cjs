const http = require('node:http');
const {buildSchema, graphql} = require(process.env.POOLSTER_GRAPHQL_JS);
const schema = buildSchema(require('node:fs').readFileSync(process.argv[2], 'utf8'));
const root = {
 node: ()=>({__typename:'Robot',id:'r1',code:'retained-second-variant'}),
 user: ({id}) => ({id, name:'Ada', nickname:null, fragile:()=>{throw new Error('field failed')}}),
 rename: ({name}) => ({id:'7',name,nickname:null}),
 fatal: ()=>{throw new Error('root failed')},
 echo: ({input})=>input,
 inputEcho: ({options})=>options===undefined?'options-omitted':options===null?'options-null':options.note,
};
const server = http.createServer(async(req,res)=>{
 if(req.url==='/disconnect'){req.socket.destroy();return;}
 if(req.url==='/http-error'){res.writeHead(503);res.end('unavailable');return;}
 if(req.url==='/bad-json'){res.end('{');return;}
 if(req.url==='/bad-envelope'){res.end('{"data":123}');return;}
 if(req.url==='/missing-nullable'){res.end('{"data":{"person":{"id":"7","name":"Ada"}}}');return;}
 if(req.url==='/null-nonnull'){res.end('{"data":{"user":{"name":null,"nickname":null}}}');return;}
 let body='';for await(const chunk of req)body+=chunk;
 const request=JSON.parse(body);
 const result=await graphql({schema,source:request.query,operationName:request.operationName,variableValues:request.variables,rootValue:root});
 res.setHeader('content-type','application/json');res.end(JSON.stringify(result));
});
server.listen(0,'127.0.0.1',()=>console.log(`http://127.0.0.1:${server.address().port}`));

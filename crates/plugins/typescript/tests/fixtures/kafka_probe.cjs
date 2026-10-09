const assert=require('node:assert/strict');
const {Kafka,logLevel}=require('kafkajs');
assert.equal(require('kafkajs/package.json').version,'2.2.4');assert.equal(require('ajv/package.json').version,'8.17.1');
(async()=>{
 const sdk=await import('./dist/index.js');
 const broker=process.env.POOLSTER_KAFKA_BROKER;
 const config={brokers:[broker],clientId:'poolster-real-test',logLevel:logLevel.NOTHING};
 const kafka=new Kafka(config);const admin=kafka.admin();await admin.connect();
 const topic='poolster-orders';
 try {await admin.deleteTopics({topics:[topic],timeout:15000});}catch{}
 await admin.createTopics({topics:[{topic,numPartitions:1,replicationFactor:1}],waitForLeaders:true});
 const client=sdk.createKafkaClient(config);let timer;
 try {
  await assert.rejects(sdk.sendOrder(client,{payload:{id:'bad',quantity:0},headers:{trace:'t'}}),sdk.KafkaSchemaError);
  await assert.rejects(sdk.sendOrder(client,{payload:{id:'bad',quantity:1},headers:{}}),sdk.KafkaSchemaError);
  await assert.rejects(sdk.receiveOrder(client,()=>{}, {groupId:'wrong'}),/conflicts/);
  let resolveMessage,resolveInvalid;
  const received=new Promise(resolve=>resolveMessage=resolve);
  const invalid=new Promise(resolve=>resolveInvalid=resolve);
  await sdk.receiveOrder(client,(message,metadata)=>resolveMessage({message,metadata}),{fromBeginning:true,onInvalidMessage:(error)=>resolveInvalid(error)});
  await sdk.sendOrder(client,{payload:{id:'42',quantity:3,note:null},key:'order-42',headers:{trace:'trace-1'}});
  const timeout=new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Kafka roundtrip timed out')),30000);});
  const result=await Promise.race([received,timeout]);
  assert.deepEqual(result.message.payload,{id:'42',quantity:3,note:null});assert.equal(result.message.key,'order-42');assert.equal(result.message.headers.trace,'trace-1');assert.equal(result.metadata.topic,topic);
  const raw=kafka.producer();await raw.connect();try {await raw.send({topic,messages:[{value:JSON.stringify({id:'bad',quantity:-1}),headers:{trace:'t'}}]});}finally{await raw.disconnect();}
  const error=await Promise.race([invalid,timeout]);assert.equal(error.name,'KafkaSchemaError');
  console.log('Generated Kafka package strict compilation + real broker roundtrip + invalid incoming/outgoing messages passed');
 }finally {clearTimeout(timer);await client.disconnect();await admin.disconnect();}
})().catch(error=>{console.error(error);process.exitCode=1;});

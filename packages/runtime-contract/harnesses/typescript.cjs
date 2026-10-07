const {KajiContract} = require(process.env.KAJI_CONTRACT_COMPILED + '/index.js')
const scenario=JSON.parse(process.env.KAJI_CONTRACT_SCENARIO??'{}')
const client = new KajiContract({baseUrl:process.env.KAJI_CONTRACT_URL, auth:{Bearer:process.env.KAJI_CONTRACT_CASE},validateResponses:scenario.validate_responses??false,middleware:[async(request,next)=>next({...request,headers:{...request.headers,'x-contract-middleware':'yes'}})]})
;(async()=>{
 const options={throwOnError:true,...('caller_key'in scenario?{headers:{'X-Once':scenario.caller_key}}:{})}
 const actions={getContact:()=>client.contacts.get(options),createContact:()=>client.contacts.create(options),patchContact:()=>client.contacts.patch(options),unsafeCreateContact:()=>client.unsafe.createContact(options),unsafePatchContact:()=>client.unsafe.patchContact(options)}
 if(scenario.action==='echoWire'){const model=await client.wire.echo({path:{key:'café/雪'},query:{text:'héllo 雪',flag:false,count:0,tags:['a','b']},headers:{'X-Label':'caller'},body:{enabled:false,count:0,note:null}});return console.log(JSON.stringify({outcome:'success',id:model.id}))}
 if(scenario.action==='listContactsPages'){
  const ids=[];for await(const page of client.contacts.listPages({query:{page:scenario.page,limit:scenario.limit}}))ids.push(...page.items.map(item=>item.id))
  return console.log(JSON.stringify({outcome:'success',id:ids.join(',')}))
 }
 let model;for(let i=0;i<(scenario.repeats??1);i++)model=await actions[scenario.action??'getContact']()
 console.log(JSON.stringify({outcome:'success',id:model.id}))
})().catch(()=>console.log(JSON.stringify({outcome:'error'})))

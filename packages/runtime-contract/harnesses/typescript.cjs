const {KajiContract} = require(process.env.KAJI_CONTRACT_COMPILED + '/index.js')
const client = new KajiContract({baseUrl:process.env.KAJI_CONTRACT_URL, auth:{Bearer:process.env.KAJI_CONTRACT_CASE},middleware:[async(request,next)=>next({...request,headers:{...request.headers,'x-contract-middleware':'yes'}})]})
client.contacts.get({throwOnError:true}).then(model=>console.log(JSON.stringify({outcome:'success',id:model.id}))).catch(()=>console.log(JSON.stringify({outcome:'error'})))

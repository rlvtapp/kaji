import { Stamp, createGraphqlHttpTransport } from './index.js';
const transport = createGraphqlHttpTransport('http://localhost');
async function check() {
 const result = await Stamp(transport, {at:'2026-10-01T00:00:00Z',include:false,input:{at:'2026-10-02T00:00:00Z',history:[],payload:{marker:'ok'}}});
 if (result.kind !== 'error') {
  const at: number = result.data.stamp.at;
  const maybe: number | null | undefined = result.data.stamp.maybe;
  const history: Array<number> = result.data.stamp.history;
  const payload: {readonly marker:string} | null = result.data.stamp.payload;
  const opaque: unknown = result.data.stamp.opaque;
  // @ts-expect-error unmapped custom scalar remains unknown
  const wrongOpaque: string = result.data.stamp.opaque;
  // @ts-expect-error mapped output direction is numeric
  const wrongDate: string = result.data.stamp.at;
  void [at,maybe,history,payload,opaque,wrongOpaque,wrongDate];
 }
 // @ts-expect-error mapped input requires wire strings
 await Stamp(transport,{at:42,include:true,input:{at:'x',history:[]}});
 // @ts-expect-error nested list input mapping remains string
 await Stamp(transport,{at:'x',include:true,input:{at:'x',history:[42]}});
 // @ts-expect-error nonnullable required scalar cannot be null
 await Stamp(transport,{at:null,include:true,input:{at:'x',history:[]}});
 await Stamp(transport,{at:'x',maybe:null,include:true,input:{at:'x',maybe:null,history:[]}});
}
void check;

using System;
using System.Linq;
using System.Text;
using System.Collections.Generic;
using ProbeSDK;
class Probe {
 static readonly byte[] Body=Encoding.UTF8.GetBytes(__BODY__);
 const string Secret=__SECRET__;
 static readonly Dictionary<string,IEnumerable<string>> Headers = new() {["webhook-id"]=new[]{"msg_test"},["webhook-timestamp"]=new[]{"1700000000"},["webhook-signature"]=new[]{__SIGNATURE__}};
 static void Reject(byte[] body,Dictionary<string,IEnumerable<string>> headers,string[] secrets,double now=1700000000,double tolerance=300) {
  try{StandardWebhooks.Verify(body,headers,secrets,now,tolerance);throw new Exception("accepted");}
  catch(StandardWebhooks.VerificationException e){if(e.Message.Contains(Secret)||e.Message.Contains("ngcu"))throw new Exception("leaked");}
 }
 static void Main() {
  var wrong="whsec_"+Convert.ToBase64String(new byte[24]);
  if(!Body.SequenceEqual(StandardWebhooks.Verify(Body,Headers,new[]{wrong,Secret},1700000000,0)))throw new Exception("vector");
  Reject(Body.Concat(new byte[]{32}).ToArray(),Headers,new[]{Secret});Reject(Body,Headers,new[]{wrong});
  Reject(Body,Headers,new[]{Secret},1700000301);Reject(Body,Headers,new[]{Secret},1699999699);
  Reject(Body,Headers,Array.Empty<string>());Reject(Body,Headers,new[]{"whsec_bad"});
  var duplicate=new Dictionary<string,IEnumerable<string>>(Headers){["Webhook-Id"]=new[]{"other"}};Reject(Body,duplicate,new[]{Secret});
  var multiple=new Dictionary<string,IEnumerable<string>>(Headers){["webhook-id"]=new[]{"one","two"}};Reject(Body,multiple,new[]{Secret});
  var rotated=new Dictionary<string,IEnumerable<string>>(Headers){["webhook-signature"]=new[]{"v1a,ignored v1,bad "+Headers["webhook-signature"].First()}};
  StandardWebhooks.Verify(Body,rotated,new[]{Secret},1700000000);
  var unsupported=new Dictionary<string,IEnumerable<string>>(Headers){["webhook-signature"]=new[]{"v1a,ignored"}};Reject(Body,unsupported,new[]{Secret});
  var model=StandardWebhooks.VerifyAndDecode<Dictionary<string,System.Text.Json.JsonElement>>(Body,Headers,new[]{Secret},1700000000);
  if(model["value"].GetInt32()!=1)throw new Exception("decode");
  try{StandardWebhooks.VerifyAndDecode<int>(Body,Headers,new[]{Secret},1700000000);throw new Exception("accepted");}catch(StandardWebhooks.VerificationException){}
 }
}

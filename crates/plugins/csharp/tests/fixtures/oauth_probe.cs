using System.Net;
using System.Text;
using ProbeSDK;
sealed class Issuer : HttpMessageHandler {
 public int Calls; public int Delay=20; public int Lifetime=3600; public bool Bad;
 protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage r,CancellationToken c){Interlocked.Increment(ref Calls);if(r.Headers.Authorization?.Scheme!="Basic")throw new Exception("basic");if(Encoding.ASCII.GetString(Convert.FromBase64String(r.Headers.Authorization.Parameter!))!="%C3%BC%3Aid:secret")throw new Exception("encoding");if(!(await r.Content!.ReadAsStringAsync(c)).Contains("grant_type=client_credentials"))throw new Exception("grant");await Task.Delay(Delay,c);return new(HttpStatusCode.OK){Content=new StringContent(Bad?"secret":$"{{\"access_token\":\"t{Calls}\",\"token_type\":\"Bearer\",\"expires_in\":{Lifetime}}}"),RequestMessage=r};}
}
sealed class Api : HttpMessageHandler {
 public int Calls; public List<string?> Tokens=new(); public bool Always;
 protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage r,CancellationToken c){Calls++;Tokens.Add(r.Headers.Authorization?.ToString());return Task.FromResult(new HttpResponseMessage(Always||Calls==1?HttpStatusCode.Unauthorized:HttpStatusCode.OK){Content=new StringContent("{}")});}
}
class Probe {
 static void Check(bool value){if(!value)throw new Exception("probe failed");}
 static async Task Main(){
  var issuer=new Issuer();var provider=new OAuthClientCredentials(new HttpClient(issuer),"https://issuer.test/token","ü:id","secret",refreshLeeway:TimeSpan.Zero);
  var tokens=await Task.WhenAll(Enumerable.Range(0,16).Select(_=>provider.TokenAsync()));Check(tokens.All(t=>t=="t1")&&issuer.Calls==1);provider.Invalidate("old");Check(await provider.TokenAsync()=="t1");
  var api=new Api();using var client=new HttpClient(new OAuthClientCredentialsHandler(provider,"https://api.test"){InnerHandler=api});using var response=await client.GetAsync("https://api.test/things");Check(response.IsSuccessStatusCode&&api.Calls==2&&api.Tokens.SequenceEqual(new[]{"Bearer t1","Bearer t2"}));
  api.Always=true;int before=api.Calls;using var denied=await client.GetAsync("https://api.test/things");Check(denied.StatusCode==HttpStatusCode.Unauthorized&&api.Calls-before==2);
  before=api.Calls;using var post=await client.PostAsync("https://api.test/things",new StringContent("body"));Check(api.Calls-before==1);
  before=api.Calls;using var outside=await client.GetAsync("https://other.test/things");Check(api.Calls-before==1&&api.Tokens.Last()==null);
  using var own=new HttpRequestMessage(HttpMethod.Get,"https://api.test/things");own.Headers.TryAddWithoutValidation("Authorization","Own token");before=api.Calls;using var ownResult=await client.SendAsync(own);Check(api.Calls-before==1&&api.Tokens.Last()=="Own token");
  var slow=new Issuer{Delay=100};var waiting=new OAuthClientCredentials(new HttpClient(slow),"https://issuer.test/token","ü:id","secret");var leader=waiting.TokenAsync();using var cancel=new CancellationTokenSource();var waiter=waiting.TokenAsync(cancel.Token);cancel.Cancel();try{await waiter;throw new Exception("cancel accepted");}catch(OperationCanceledException){}Check(await leader=="t1"&&slow.Calls==1);
  var zero=new Issuer{Lifetime=0};var uncached=new OAuthClientCredentials(new HttpClient(zero),"https://issuer.test/token","ü:id","secret");await uncached.TokenAsync();await uncached.TokenAsync();Check(zero.Calls==2);
  var bad=new Issuer{Bad=true};var invalid=new OAuthClientCredentials(new HttpClient(bad),"https://issuer.test/token","ü:id","secret");try{await invalid.TokenAsync();throw new Exception("bad accepted");}catch(OAuthTokenException e){Check(!e.Message.Contains("secret"));}
 }
}

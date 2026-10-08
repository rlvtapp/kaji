using System.Net;using Kaji.KajiScope;
class Driver:HttpMessageHandler {
 public int Calls;public int Delay;public List<string?> Scopes=new();public List<string?> Auth=new();
 protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token){Calls++;Scopes.Add(request.Headers.TryGetValues("X-Scope",out var scopes)?scopes.Single():null);Auth.Add(request.Headers.TryGetValues("Authorization",out var values)?values.Single():null);if(Delay>0)await Task.Delay(Delay,token);return new(HttpStatusCode.OK){Content=new StringContent("{}")};}
}
class Probe {static void Check(bool x){if(!x)throw new Exception("scope probe");}static async Task Main(){
 var driver=new Driver();var client=new KajiClient(new HttpClient(driver),new KajiClientOptions{BaseUrl="https://api.test",ApiKey="static",Retry=new(){MaxAttempts=1}});var headers=new Dictionary<string,string>{{"X-Scope","scope"},{"Authorization","Own scope"}};var scoped=client.ForCall(new KajiCallOptions{Headers=headers,Timeout=TimeSpan.FromMilliseconds(25)});headers["X-Scope"]="changed";
 await scoped.GetThingAsync();await client.GetThingAsync();Check(driver.Scopes.SequenceEqual(new string?[]{"scope",null}));Check(driver.Auth.SequenceEqual(new[]{"Own scope","Bearer static"}));
 driver.Delay=100;int before=driver.Calls;try{await scoped.GetThingAsync();throw new Exception("timeout ignored");}catch(OperationCanceledException){}Check(driver.Calls-before==1);driver.Delay=0;await client.GetThingAsync();Check(driver.Scopes.Last()==null);
 try{client.ForCall(new KajiCallOptions{Timeout=TimeSpan.Zero});throw new Exception("invalid timeout accepted");}catch(ArgumentOutOfRangeException){}
}}

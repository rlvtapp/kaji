using System.Net;
using System.Text;
using System.Text.Json;
using Probe;
sealed class Driver(string? target=null) : HttpMessageHandler {
    public List<string> URLs { get; }=[];
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token) {
        token.ThrowIfCancellationRequested();
        if(request.Headers.Authorization?.ToString()!="Bearer test") throw new Exception("Lost auth");
        URLs.Add(request.RequestUri!.AbsoluteUri);
        var next=target ?? (request.RequestUri.Query.Contains("page=2")?null:"https://api.example.test/links?page=2&tenant=server");
        return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK){Content=new StringContent(JsonSerializer.Serialize(new {next}),Encoding.UTF8,"application/json")});
    }
}
static class Program {
    static KajiClient Client(Driver driver)=>new(new HttpClient(driver),new KajiClientOptions{BaseUrl="https://api.example.test",ApiKey="test",Retry=new KajiRetryOptions{MaxAttempts=1}});
    static async Task Main() {
        var driver=new Driver();var client=Client(driver);var pages=client.ListLinksPagesAsync(tenant:"kept");if(driver.URLs.Count!=0)throw new Exception("Eager");
        var count=0;await foreach(var _ in pages)count++;if(count!=2||!driver.URLs[0].Contains("tenant=kept")||driver.URLs[1]!="https://api.example.test/links?page=2&tenant=server")throw new Exception("Queries changed");
        foreach(var target in new[]{"https://evil.example/links","http://api.example.test/links","https://api.example.test:444/links","https://user@api.example.test/links","https://api.example.test/links#fragment","/relative"}) {
            driver=new Driver(target);client=Client(driver);bool rejected=false;try {await foreach(var _ in client.ListLinksPagesAsync()) {}}catch(InvalidOperationException){rejected=true;}
            if(!rejected||driver.URLs.Count!=1)throw new Exception("Unsafe origin reached transport");
        }
        driver=new Driver();client=Client(driver);using var cancellation=new CancellationTokenSource();cancellation.Cancel();
        bool cancelled=false;try{await foreach(var _ in client.ListLinksPagesAsync(cancellationToken:cancellation.Token)) {}}catch(OperationCanceledException){cancelled=true;}
        if(!cancelled||driver.URLs.Count!=0)throw new Exception("Cancellation was eager");
    }
}

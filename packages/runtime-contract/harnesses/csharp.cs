using System.Text.Json;
using Kaji.ContractSdk;

using var native = new HttpClient(new Policy { InnerHandler = new HttpClientHandler() });
var client = new KajiClient(native, new KajiClientOptions { BaseUrl = Environment.GetEnvironmentVariable("KAJI_CONTRACT_URL")!, ApiKey = Environment.GetEnvironmentVariable("KAJI_CONTRACT_CASE") });
try { var model = await client.GetContactAsync(); Console.WriteLine(JsonSerializer.Serialize(new { outcome = "success", id = model.Id })); }
catch (Exception) { Console.WriteLine("{\"outcome\":\"error\"}"); }

sealed class Policy : DelegatingHandler {
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellation) {
        request.Headers.Add("X-Contract-Middleware", "yes");
        return base.SendAsync(request, cancellation);
    }
}

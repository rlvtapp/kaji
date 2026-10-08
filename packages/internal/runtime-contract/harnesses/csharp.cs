using System.Text.Json;
using Poolster.ContractSdk;

using var native = new HttpClient(new Policy { InnerHandler = new HttpClientHandler() });
var client = new PoolsterClient(native, new PoolsterClientOptions { BaseUrl = Environment.GetEnvironmentVariable("KAJI_CONTRACT_URL")!, ApiKey = Environment.GetEnvironmentVariable("KAJI_CONTRACT_CASE") });
using var document = JsonDocument.Parse(Environment.GetEnvironmentVariable("KAJI_CONTRACT_SCENARIO") ?? "{}");
var scenario = document.RootElement;
var action = scenario.TryGetProperty("action", out var actionValue) ? actionValue.GetString() : "getContact";
var callerKey = scenario.TryGetProperty("caller_key", out var keyValue) ? keyValue.GetString() : null;
try {
    string? id = null;
    var repeats = scenario.TryGetProperty("repeats", out var repeated) ? repeated.GetInt32() : 1;
    for (var call = 0; call < repeats; call++) {
        switch (action) {
            case "getContact": id = (await client.GetContactAsync()).Id; break;
            case "createContact": id = (await client.CreateContactAsync(xOnce: callerKey)).Id; break;
            case "patchContact": id = (await client.PatchContactAsync(xOnce: callerKey)).Id; break;
            case "unsafeCreateContact": id = (await client.UnsafeCreateContactAsync()).Id; break;
            case "unsafePatchContact": id = (await client.UnsafePatchContactAsync()).Id; break;
            case "echoWire": id = (await client.EchoWireAsync(key: "café/雪", body: new WireInput { Enabled = false, Count = 0, Note = null }, text: "héllo 雪", flag: false, count: 0, tags: new List<string> { "a", "b" }, xLabel: "caller")).Id; break;
            case "listContactsPages":
                var ids = new List<string>();
                long? page = scenario.TryGetProperty("page", out var pageValue) ? pageValue.GetInt64() : null;
                long? limit = scenario.TryGetProperty("limit", out var limitValue) ? limitValue.GetInt64() : null;
                await foreach (var response in client.ListContactsPagesAsync(page: page, limit: limit))
                    ids.AddRange(response.Items.Select(contact => contact.Id));
                id = string.Join(",", ids); break;
            default: throw new InvalidOperationException("unsupported contract action");
        }
    }
    Console.WriteLine(JsonSerializer.Serialize(new { outcome = "success", id }));
} catch (Exception) { Console.WriteLine("{\"outcome\":\"error\"}"); }

sealed class Policy : DelegatingHandler {
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellation) {
        request.Headers.Add("X-Contract-Middleware", "yes");
        return base.SendAsync(request, cancellation);
    }
}

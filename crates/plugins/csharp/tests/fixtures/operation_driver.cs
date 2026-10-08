using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using __PACKAGE__;

/// Generated smoke tests: every HTTP request is handled in memory.
static class PoolsterOperationTests {
    static readonly JsonSerializerOptions Options = new() { DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull };
    static void Check(bool value,string reason){if(!value)throw new Exception(reason);}
    static bool Same(JsonElement value,JsonElement expected)=>JsonNode.DeepEquals(JsonNode.Parse(value.GetRawText()),JsonNode.Parse(expected.GetRawText()));
    sealed class Driver:HttpMessageHandler {
        public readonly JsonElement Fixture;public int Calls;
        public Driver(string fixture){Fixture=JsonDocument.Parse(fixture).RootElement.Clone();}
        static string Scalar(JsonElement value)=>value.ValueKind==JsonValueKind.String?value.GetString()!:value.GetRawText();
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token){
            token.ThrowIfCancellationRequested();Calls++;
            Check(request.Method.Method==Fixture.GetProperty("method").GetString(),"request method");
            Check(request.RequestUri!.AbsolutePath==Fixture.GetProperty("encoded_path").GetString(),"request path");
            var query = new Dictionary<string,string>();
            foreach(var entry in request.RequestUri.Query.TrimStart('?').Split('&',StringSplitOptions.RemoveEmptyEntries)){
                var pair=entry.Split('=',2);Check(pair.Length==2,"query pair");
                query[Uri.UnescapeDataString(pair[0].Replace("+"," "))]=Uri.UnescapeDataString(pair[1].Replace("+"," "));
            }
            foreach(var parameter in Fixture.GetProperty("parameters").EnumerateArray()){
                var name=parameter.GetProperty("name").GetString()!;var value=Scalar(parameter.GetProperty("value"));
                if(parameter.GetProperty("location").GetString()=="query")Check(query.GetValueOrDefault(name)==value,"query value");
                if(parameter.GetProperty("location").GetString()=="header")Check(request.Headers.TryGetValues(name,out var values)&&values.Single()==value,"header value");
            }
            if(Fixture.GetProperty("has_body").GetBoolean()){
                Check(request.Content is not null,"missing body");
                using var body=JsonDocument.Parse(await request.Content!.ReadAsStringAsync(token));
                Check(Same(body.RootElement,Fixture.GetProperty("body")),"request body");
            }else Check(request.Content is null,"unexpected body");
            return new HttpResponseMessage((HttpStatusCode)Fixture.GetProperty("status").GetInt32()){
                Content=new StringContent(Fixture.GetProperty("response").GetRawText(),Encoding.UTF8,"application/json")
            };
        }
    }
    static void AssertResult(Driver driver,object? result){
        Check(driver.Calls==1,"one request required");
        Check(Same(JsonSerializer.SerializeToElement(result,Options),driver.Fixture.GetProperty("response")),"response round trip");
    }
    static async Task Main(){
__CASES__
    }
}

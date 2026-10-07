import contract.sdk.Client;
import contract.sdk.ClientConfig;
import contract.sdk.model.WireInput;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.net.*;
import java.net.http.*;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.*;
import javax.net.ssl.*;

class RuntimeContract {
    static final class Policy extends HttpClient {
        final HttpClient next = HttpClient.newHttpClient();
        HttpRequest rewrite(HttpRequest request) { return HttpRequest.newBuilder(request, (name,value)->true).header("X-Contract-Middleware","yes").build(); }
        public <T> HttpResponse<T> send(HttpRequest request,HttpResponse.BodyHandler<T> handler) throws java.io.IOException,InterruptedException { return next.send(rewrite(request),handler); }
        public <T> CompletableFuture<HttpResponse<T>> sendAsync(HttpRequest request,HttpResponse.BodyHandler<T> handler) { return next.sendAsync(rewrite(request),handler); }
        public <T> CompletableFuture<HttpResponse<T>> sendAsync(HttpRequest request,HttpResponse.BodyHandler<T> handler,HttpResponse.PushPromiseHandler<T> push) { return next.sendAsync(rewrite(request),handler,push); }
        public Optional<CookieHandler> cookieHandler(){return next.cookieHandler();}
        public Optional<Duration> connectTimeout(){return next.connectTimeout();}
        public Redirect followRedirects(){return next.followRedirects();}
        public Optional<ProxySelector> proxy(){return next.proxy();}
        public SSLContext sslContext(){return next.sslContext();}
        public SSLParameters sslParameters(){return next.sslParameters();}
        public Optional<Authenticator> authenticator(){return next.authenticator();}
        public Version version(){return next.version();}
        public Optional<Executor> executor(){return next.executor();}
    }
    public static void main(String[] args) throws Exception {
        var config = new ClientConfig(System.getenv("KAJI_CONTRACT_URL"),System.getenv("KAJI_CONTRACT_CASE"),"Authorization","Bearer",Map.of(),new Policy(),Duration.ofSeconds(10),null,null);
        var client = new Client(config);
        var mapper = new ObjectMapper();
        var scenario = mapper.readTree(Optional.ofNullable(System.getenv("KAJI_CONTRACT_SCENARIO")).orElse("{}"));
        var action = scenario.path("action").asText("getContact");
        String callerKey = scenario.hasNonNull("caller_key") ? scenario.get("caller_key").asText() : null;
        try {
            String id = null;
            for (int call = 0; call < scenario.path("repeats").asInt(1); call++) {
                switch (action) {
                    case "getContact": id = client.getContact().id(); break;
                    case "createContact": id = client.createContact(new Client.CreateContactRequest(callerKey)).id(); break;
                    case "patchContact": id = client.patchContact(new Client.PatchContactRequest(callerKey)).id(); break;
                    case "unsafeCreateContact": id = client.unsafeCreateContact().id(); break;
                    case "unsafePatchContact": id = client.unsafePatchContact().id(); break;
                    case "echoWire": id = client.echoWire(new Client.EchoWireRequest("café/雪", "héllo 雪", false, 0L, List.of("a", "b"), "caller", new WireInput(false, 0L, null, null))).id(); break;
                    case "listContactsPages":
                        var ids = new ArrayList<String>();
                        for (var page : client.listContactsPages(new Client.ListContactsRequest(scenario.path("page").asLong(1), scenario.path("limit").asLong(2))))
                            for (var contact : page.items()) ids.add(contact.id());
                        id = String.join(",", ids); break;
                    default: throw new IllegalArgumentException("unsupported contract action");
                }
            }
            System.out.println(mapper.writeValueAsString(Map.of("outcome", "success", "id", id)));
        } catch (RuntimeException error) { System.out.println("{\"outcome\":\"error\"}"); }
    }
}

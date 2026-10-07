import contract.sdk.Client;
import contract.sdk.ClientConfig;
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
        try { var model=client.getContact();System.out.println("{\"outcome\":\"success\",\"id\":\""+model.id()+"\"}"); }
        catch(RuntimeException error){System.out.println("{\"outcome\":\"error\"}");}
    }
}

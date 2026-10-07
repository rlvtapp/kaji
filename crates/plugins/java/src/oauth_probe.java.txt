package io.kaji.oauth;
import java.net.*;import java.net.http.*;import java.time.Duration;import java.util.*;import java.util.concurrent.*;import java.util.concurrent.atomic.AtomicInteger;import java.nio.ByteBuffer;import javax.net.ssl.*;
public final class OAuthProbe {
 static void check(boolean value){if(!value)throw new AssertionError("OAuth probe");}
 static final class Driver extends HttpClient {
  final boolean issuer; final AtomicInteger calls=new AtomicInteger();final List<String> tokens=Collections.synchronizedList(new ArrayList<>());boolean always,bad;int lifetime=3600,delay=10;
  Driver(boolean issuer){this.issuer=issuer;}
  public <T> HttpResponse<T> send(HttpRequest request,HttpResponse.BodyHandler<T> handler)throws java.io.IOException,InterruptedException{
   int count=calls.incrementAndGet();int status;byte[] bytes;
   if(issuer){check(request.headers().firstValue("Authorization").orElse("").equals("Basic "+Base64.getEncoder().encodeToString("%C3%BC%3Aid:secret".getBytes())));Thread.sleep(delay);status=200;bytes=(bad?"secret":"{\"access_token\":\"t"+count+"\",\"token_type\":\"Bearer\",\"expires_in\":"+lifetime+"}").getBytes();}
   else{tokens.add(request.headers().firstValue("Authorization").orElse(""));status=always||count==1?401:200;bytes="{}".getBytes();}
   final int code=status;var headers=HttpHeaders.of(Map.of("content-type",List.of("application/json")),(a,b)->true);
   var subscriber=handler.apply(new HttpResponse.ResponseInfo(){public int statusCode(){return code;}public HttpHeaders headers(){return headers;}public Version version(){return Version.HTTP_1_1;}});
   subscriber.onSubscribe(new Flow.Subscription(){public void request(long n){}public void cancel(){}});subscriber.onNext(List.of(ByteBuffer.wrap(bytes)));subscriber.onComplete();T body=subscriber.getBody().toCompletableFuture().join();
   return new HttpResponse<T>(){public int statusCode(){return code;}public HttpRequest request(){return request;}public Optional<HttpResponse<T>> previousResponse(){return Optional.empty();}public HttpHeaders headers(){return headers;}public T body(){return body;}public Optional<SSLSession> sslSession(){return Optional.empty();}public URI uri(){return request.uri();}public Version version(){return Version.HTTP_1_1;}};
  }
  public <T> CompletableFuture<HttpResponse<T>> sendAsync(HttpRequest r,HttpResponse.BodyHandler<T> h){throw new UnsupportedOperationException();}public <T> CompletableFuture<HttpResponse<T>> sendAsync(HttpRequest r,HttpResponse.BodyHandler<T> h,HttpResponse.PushPromiseHandler<T> p){throw new UnsupportedOperationException();}
  public Optional<CookieHandler> cookieHandler(){return Optional.empty();}public Optional<Duration> connectTimeout(){return Optional.empty();}public Redirect followRedirects(){return Redirect.NEVER;}public Optional<ProxySelector> proxy(){return Optional.empty();}public SSLContext sslContext(){try{return SSLContext.getDefault();}catch(Exception e){throw new RuntimeException();}}public SSLParameters sslParameters(){return new SSLParameters();}public Optional<Authenticator> authenticator(){return Optional.empty();}public Version version(){return Version.HTTP_1_1;}public Optional<Executor> executor(){return Optional.empty();}
 }
 public static void main(String[] args)throws Exception{
  var issuer=new Driver(true);var provider=new OAuthClientCredentials(issuer,"https://issuer.test/token","ü:id","secret",List.of(),Duration.ZERO,null);
  var executor=Executors.newFixedThreadPool(16);try{var futures=new ArrayList<Future<String>>();for(int i=0;i<16;i++)futures.add(executor.submit(()->provider.token()));for(var future:futures)check(future.get().equals("t1"));check(issuer.calls.get()==1);}finally{executor.shutdownNow();}
  provider.invalidate("old");check(provider.token().equals("t1"));var driver=new Driver(false);var client=new OAuthHttpClient(driver,provider,"https://api.test");var get=HttpRequest.newBuilder(URI.create("https://api.test/things")).GET().build();check(client.send(get,HttpResponse.BodyHandlers.ofString()).statusCode()==200);check(driver.tokens.equals(List.of("Bearer t1","Bearer t2")));
  driver.always=true;int before=driver.calls.get();check(client.send(get,HttpResponse.BodyHandlers.ofString()).statusCode()==401);check(driver.calls.get()-before==2);
  before=driver.calls.get();client.send(HttpRequest.newBuilder(get.uri()).POST(HttpRequest.BodyPublishers.ofString("body")).build(),HttpResponse.BodyHandlers.ofString());check(driver.calls.get()-before==1);
  client.send(HttpRequest.newBuilder(URI.create("https://other.test/things")).build(),HttpResponse.BodyHandlers.ofString());check(driver.tokens.get(driver.tokens.size()-1).isEmpty());
  client.send(HttpRequest.newBuilder(get.uri()).header("Authorization","Own token").build(),HttpResponse.BodyHandlers.ofString());check(driver.tokens.get(driver.tokens.size()-1).equals("Own token"));
  var zero=new Driver(true);zero.lifetime=0;var uncached=new OAuthClientCredentials(zero,"https://issuer.test/token","ü:id","secret");uncached.token();uncached.token();check(zero.calls.get()==2);
  var bad=new Driver(true);bad.bad=true;try{new OAuthClientCredentials(bad,"https://issuer.test/token","ü:id","secret").token();throw new AssertionError();}catch(java.io.IOException error){check(!error.toString().contains("secret"));}
  var slow=new Driver(true);slow.delay=100;var waiting=new OAuthClientCredentials(slow,"https://issuer.test/token","ü:id","secret");var leader=new Thread(()->{try{waiting.token();}catch(Exception e){throw new RuntimeException(e);}});leader.start();Thread.sleep(10);var canceled=new AtomicInteger();var waiter=new Thread(()->{try{waiting.token();}catch(InterruptedException e){canceled.incrementAndGet();}catch(Exception e){throw new RuntimeException(e);}});waiter.start();Thread.sleep(10);waiter.interrupt();waiter.join();leader.join();check(canceled.get()==1&&slow.calls.get()==1);
 }
}

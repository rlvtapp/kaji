package __PACKAGE__
import("context";"encoding/json";"errors";"fmt";"io";"net/http";"os";"strings";"sync";"sync/atomic";"testing";"time")
type securityDoFunc func(*http.Request)(*http.Response,error)
func(f securityDoFunc)Do(r *http.Request)(*http.Response,error){return f(r)}
func securityReply(status int,body string)*http.Response{return &http.Response{StatusCode:status,Header:http.Header{"Content-Type":{"application/json"}},Body:io.NopCloser(strings.NewReader(body))}}
func TestWebhookRawRotationToleranceAndRedaction(t *testing.T){
 var vector struct{Secret string `json:"secret"`;RawBody string `json:"raw_body"`;Headers map[string]string `json:"headers"`;Now int64 `json:"now"`;Payload json.RawMessage `json:"payload"`}
 bytes,_:=os.ReadFile("vector.json");if err:=json.Unmarshal(bytes,&vector);err!=nil{t.Fatal(err)}
 headers:=http.Header{};for key,value:=range vector.Headers{headers.Set(key,value)}
 options:=WebhookVerificationOptions{Now:time.Unix(vector.Now,0),Tolerance:5*time.Minute}
 verified,err:=VerifyWebhook([]byte(vector.RawBody),headers,[]string{vector.Secret},options);if err!=nil||string(verified)!=vector.RawBody{t.Fatalf("official-compatible vector failed: %v",err)}
 decoded,err:=VerifyWebhookAndDecode[struct{Value int `json:"value"`}]([]byte(vector.RawBody),headers,[]string{vector.Secret},options);if err!=nil||decoded.Value!=1{t.Fatal(err)}
 rotated:=headers.Clone();rotated.Set("webhook-signature","v2,ignored v1,broken "+headers.Get("webhook-signature"));if _,err=VerifyWebhook([]byte(vector.RawBody),rotated,[]string{"whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4",vector.Secret},options);err!=nil{t.Fatal(err)}
 cases:=[]struct{body string;headers http.Header;secrets []string;options WebhookVerificationOptions}{{vector.RawBody+" ",headers,[]string{vector.Secret},options},{vector.RawBody,headers,[]string{vector.Secret},WebhookVerificationOptions{Now:time.Unix(vector.Now+301,0),Tolerance:300*time.Second}},{vector.RawBody,headers,[]string{vector.Secret},WebhookVerificationOptions{Now:time.Unix(vector.Now-301,0),Tolerance:300*time.Second}},{vector.RawBody,headers,[]string{"whsec_bad"},options},{vector.RawBody,headers,nil,options}}
 duplicate:=headers.Clone();duplicate["WEBHOOK-ID"]=[]string{"other"};cases=append(cases,struct{body string;headers http.Header;secrets []string;options WebhookVerificationOptions}{vector.RawBody,duplicate,[]string{vector.Secret},options})
 unsupported:=headers.Clone();unsupported.Set("webhook-signature","v1a,"+strings.TrimPrefix(headers.Get("webhook-signature"),"v1,"));cases=append(cases,struct{body string;headers http.Header;secrets []string;options WebhookVerificationOptions}{vector.RawBody,unsupported,[]string{vector.Secret},options})
 for _,sample:=range cases{_,err=VerifyWebhook([]byte(sample.body),sample.headers,sample.secrets,sample.options);if err==nil||strings.Contains(err.Error(),vector.Secret)||strings.Contains(err.Error(),vector.RawBody){t.Fatal("bad webhook accepted or secret exposed")}}
}
func TestOAuthConcurrentRefreshCacheAndCancellation(t *testing.T){
 var calls atomic.Int32
 issuer:=securityDoFunc(func(request *http.Request)(*http.Response,error){
  id,secret,ok:=request.BasicAuth();if !ok||id!="a%2Bb"||secret!="s%3Ae"{t.Fatal("OAuth credentials not individually encoded")}
  if err:=request.ParseForm();err!=nil||request.Form.Get("grant_type")!="client_credentials"||request.Form.Get("scope")!="read write"{t.Fatal("bad OAuth form")}
  count:=calls.Add(1);return securityReply(200,fmt.Sprintf(`{"access_token":"token%d","token_type":"Bearer","expires_in":3600}`,count)),nil
 })
 provider,err:=NewOAuthClientCredentials(OAuthClientCredentialsConfig{TokenURL:"https://issuer.invalid/token",ClientID:"a+b",ClientSecret:"s:e",Scopes:[]string{"read","write"},HTTPClient:issuer});if err!=nil{t.Fatal(err)}
 for index,rejected:=range []string{"","token1"}{var wait sync.WaitGroup;for n:=0;n<16;n++{wait.Add(1);go func(){defer wait.Done();token,err:=provider.Token(context.Background(),rejected);if err!=nil||token!=fmt.Sprintf("token%d",index+1){t.Error("cache coordination failed",err)}}()};wait.Wait()}
 if calls.Load()!=2{t.Fatal("stampede",calls.Load())};if token,_:=provider.Token(context.Background(),"token1");token!="token2"||calls.Load()!=2{t.Fatal("obsolete rejection invalidated new token")}
 cancelled,cancel:=context.WithCancel(context.Background());cancel();if _,err=provider.Token(cancelled,"token2");!errors.Is(err,context.Canceled){t.Fatal("cancellation lost")}
 started:=make(chan struct{});blocked:=securityDoFunc(func(request *http.Request)(*http.Response,error){close(started);<-request.Context().Done();return nil,request.Context().Err()})
 provider.config.HTTPClient=blocked
 leaderCtx,leaderCancel:=context.WithCancel(context.Background());leaderDone:=make(chan error,1);go func(){_,err:=provider.Token(leaderCtx,"token2");leaderDone<-err}();<-started
 waiterCtx,waiterCancel:=context.WithCancel(context.Background());waiterCancel();if _,err=provider.Token(waiterCtx,"token2");!errors.Is(err,context.Canceled){t.Fatal("waiter cancellation lost")};leaderCancel();if err=<-leaderDone;!errors.Is(err,context.Canceled){t.Fatal("leader cancellation lost",err)}
 provider.config.HTTPClient=issuer;if token,err:=provider.Token(context.Background(),"token2");err!=nil||token!="token3"{t.Fatal("cancelled refresh poisoned cache",err)}
}
func TestOAuthSafeBounded401ReplayAndExplicitAuthorization(t *testing.T){
 var issued atomic.Int32;issuer:=securityDoFunc(func(*http.Request)(*http.Response,error){count:=issued.Add(1);return securityReply(200,fmt.Sprintf(`{"access_token":"token%d","expires_in":3600}`,count)),nil})
 provider,_:=NewOAuthClientCredentials(OAuthClientCredentialsConfig{TokenURL:"https://issuer.invalid/token",ClientID:"id",ClientSecret:"secret",HTTPClient:issuer})
 var attempts int;driver:=securityDoFunc(func(request *http.Request)(*http.Response,error){attempts++;if request.Header.Get("Authorization")==""{t.Fatal("missing bearer")};return securityReply(401,`{}`),nil})
 client,err:=NewClient(ClientConfig{BaseURL:"https://api.invalid",HTTPClient:driver,TokenProvider:provider,Retry:&RetryConfig{MaxAttempts:10}});if err!=nil{t.Fatal(err)}
 request,_:=client.newRequest(context.Background(),http.MethodGet,"/contacts",nil,nil,nil);if err=client.doWithRetry(request,nil);err==nil||attempts!=2||issued.Load()!=2{t.Fatal("401 was not bounded",attempts,issued.Load(),err)}
 for _,method:=range []string{http.MethodPost,http.MethodPatch}{attempts=0;request,_=client.newRequest(context.Background(),method,"/contacts",nil,nil,map[string]string{"x":"y"});before:=issued.Load();_ =client.doWithRetry(request,nil);if attempts!=1||issued.Load()!=before{t.Fatal("unsafe mutation replayed",method)}}
 attempts=0;request,_=client.newRequest(context.Background(),http.MethodGet,"/contacts",nil,http.Header{"Authorization":{"custom"}},nil);before:=issued.Load();_ =client.doWithRetry(request,nil);if attempts!=1||issued.Load()!=before{t.Fatal("explicit authorization replaced")}
}

func TestOAuthExpiryMalformedResponsesAndErrorsStayPrivate(t *testing.T){
 now:=time.Unix(1700000000,0);calls:=0
 issuer:=securityDoFunc(func(*http.Request)(*http.Response,error){calls++;return securityReply(200,fmt.Sprintf(`{"access_token":"token%d","expires_in":60}`,calls)),nil})
 provider,_:=NewOAuthClientCredentials(OAuthClientCredentialsConfig{TokenURL:"https://issuer.invalid/token",ClientID:"id",ClientSecret:"PRIVATE_SECRET",HTTPClient:issuer});provider.now=func()time.Time{return now}
 if token,err:=provider.Token(context.Background(),"");err!=nil||token!="token1"{t.Fatal(err)}
 now=now.Add(29*time.Second);if _,err:=provider.Token(context.Background(),"");err!=nil||calls!=1{t.Fatal("short lifetime immediately refreshed",err)}
 now=now.Add(2*time.Second);if token,err:=provider.Token(context.Background(),"");err!=nil||token!="token2"{t.Fatal("expiry did not refresh",err)}
 for _,body:=range []string{`{"error":"PRIVATE_SECRET"}`,`{"access_token":"PRIVATE_SECRET","token_type":"MAC","expires_in":60}`,`{"access_token":"PRIVATE_SECRET","expires_in":-1}`,`{"access_token":"PRIVATE_SECRET","expires_in":1e100}`,`{"access_token":"PRIVATE_SECRET","expires_in":null}`,strings.Repeat("x",(1<<20)+1)}{
  provider.config.HTTPClient=securityDoFunc(func(*http.Request)(*http.Response,error){return securityReply(200,body),nil})
  _,err:=provider.Token(context.Background(),"token2");if err==nil||strings.Contains(err.Error(),"PRIVATE_SECRET"){t.Fatal("invalid token response accepted or exposed",err)}
 }
}

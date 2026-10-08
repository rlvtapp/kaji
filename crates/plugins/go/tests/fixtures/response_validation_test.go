package email
import("context";"encoding/json";"errors";"io";"net/http";"strings";"testing";"time")
type trackedBody struct { io.Reader; closed int }
func(body *trackedBody) Close() error { body.closed++;return nil }
type lifecycle struct { before,after,failures int }
func(hook *lifecycle) BeforeRequest(PoolsterRequestInfo) { hook.before++ }
func(hook *lifecycle) AfterResponse(PoolsterResponseInfo) { hook.after++ }
func(hook *lifecycle) OnError(PoolsterRequestInfo,error) { hook.failures++ }
func response(body io.ReadCloser,status int) *http.Response { return &http.Response{StatusCode:status,Header:http.Header{"Content-Type":[]string{"application/json"}},Body:body} }
func TestValidateResponses(t *testing.T) {
    for _,body := range []string{`{}`,`null`,`{"email":null}`,`{"email":false}`,`{"email":"future","display_name":null}`,`{"email":"future","children":[{}]}`,`{"email":"future","children":[null]}`,`{"email":"future","labels":{"count":null}}`,`{"email":"future","labels":{"count":"secret value"}}`,`{"email":"future"} {}`,``} {
        t.Run(body,func(t *testing.T){
            tracked := &trackedBody{Reader:strings.NewReader(body)}
            hooks := &lifecycle{}
            client,err := NewClient(ClientConfig{BaseURL:"https://example.test",ValidateResponses:true,Hooks:hooks,HTTPClient:PoolsterHTTPClientFunc(func(*http.Request)(*http.Response,error){return response(tracked,200),nil})});if err!=nil{t.Fatal(err)}
            request,_:=client.newRequest(context.Background(),"GET","/",nil,nil,nil)
            var model Contact
            err=client.doWithRetry(request,&model)
            var validation *ResponseValidationError
            if err!=nil&&strings.Contains(err.Error(),"secret value"){t.Fatal("response value leaked",err)}
            if !errors.As(err,&validation){t.Fatalf("expected structural error, got %v",err)}
            if tracked.closed!=1||hooks.before!=1||hooks.after!=1||hooks.failures!=1{t.Fatalf("body/lifecycle: %#v %#v",tracked,hooks)}
        })
    }
    client,_:=NewClient(ClientConfig{BaseURL:"https://example.test",ValidateResponses:true})
    var model Contact
    if err:=client.decodeResponse(strings.NewReader(`{"email":"future","extra":{"wide":9007199254740993},"children":[{"email":"next"}]}`),&model);err!=nil{t.Fatal(err)}
    encoded,err:=json.Marshal(model);if err!=nil||!strings.Contains(string(encoded),"9007199254740993"){t.Fatalf("unknown properties lost: %s %v",encoded,err)}
    var models []Contact
    if err:=client.decodeResponse(strings.NewReader(`[{"email":"ok"},{}]`),&models);err==nil{t.Fatal("array model validation skipped")}
    var scalar int64
    if err:=client.decodeResponse(strings.NewReader(`null`),&scalar);err==nil{t.Fatal("scalar null accepted")}
    var anonymous []string
    if err:=client.decodeResponse(strings.NewReader(`{}`),&anonymous);err==nil{t.Fatal("array shape skipped")}
    var nullable *int64
    if err:=client.decodeResponse(strings.NewReader(`null`),&nullable);err!=nil{t.Fatal("native nullable pointer rejected",err)}
    legacy,_:=NewClient(ClientConfig{BaseURL:"https://example.test"})
    if err:=legacy.decodeResponse(strings.NewReader(`{}`),&model);err!=nil{t.Fatal("default compatibility changed",err)}
    if err:=client.decodeResponse(strings.NewReader(strings.Repeat(" ",10<<20)+`{}`),&model);err==nil{t.Fatal("body limit skipped")}
}
func TestMiddlewareAttemptsAndResponseRewrite(t *testing.T) {
    attempts:=0; wrappers:=0; hooks:=&lifecycle{}
    first:=&trackedBody{Reader:strings.NewReader(`retry`)}; discarded:=&trackedBody{Reader:strings.NewReader(`{}`)}; replacement:=&trackedBody{Reader:strings.NewReader(`{"email":"rewritten"}`)}
    native:=PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){attempts++;if attempts==1{return response(first,503),nil};return response(discarded,200),nil})
    middleware:=func(next PoolsterHTTPClient) PoolsterHTTPClient{return PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){wrappers++;r,e:=next.Do(request);if e==nil&&r.StatusCode==200{r.Body.Close();r.Body=replacement};return r,e})}
    client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:native,Middleware:[]PoolsterMiddleware{middleware},Hooks:hooks,ValidateResponses:true,Retry:&RetryConfig{MaxAttempts:2,InitialDelay:time.Nanosecond,MaxDelay:time.Nanosecond}});if err!=nil{t.Fatal(err)}
    request,_:=client.newRequest(context.Background(),"GET","/",nil,nil,nil);var model Contact
    if err:=client.doWithRetry(request,&model);err!=nil||model.Email!="rewritten"{t.Fatalf("%#v %v",model,err)}
    if attempts!=2||wrappers!=2||hooks.before!=1||hooks.after!=1||hooks.failures!=0||first.closed!=1||discarded.closed!=1||replacement.closed!=1{t.Fatalf("attempt/body/hook mismatch: %d %d %#v %d %d %d",attempts,wrappers,hooks,first.closed,discarded.closed,replacement.closed)}
}
func TestCancellationAndStreamOwnership(t *testing.T) {
    ctx,cancel:=context.WithCancel(context.Background());hooks:=&lifecycle{};attempts:=0
    body:=&trackedBody{Reader:strings.NewReader("retry")}
    transport:=PoolsterHTTPClientFunc(func(*http.Request)(*http.Response,error){attempts++;cancel();return response(body,503),nil})
    client,_:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:transport,Hooks:hooks})
    request,_:=client.newRequest(ctx,"GET","/",nil,nil,nil)
    if err:=client.doWithRetry(request,nil);!errors.Is(err,context.Canceled){t.Fatal(err)}
    if attempts!=1||body.closed!=1||hooks.before!=1||hooks.failures!=1{t.Fatalf("cancellation lifecycle %#v %d",hooks,attempts)}
    if err:=client.doWithRetry(request,nil);!errors.Is(err,context.Canceled)||attempts!=1{t.Fatalf("pre-cancelled attempt executed: %v %d",err,attempts)}
    streamBody:=&trackedBody{Reader:strings.NewReader("data: hello\n\n")};wrappers:=0
    wrapper:=func(next PoolsterHTTPClient) PoolsterHTTPClient{return PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){wrappers++;return next.Do(request)})}
    client,_=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:PoolsterHTTPClientFunc(func(*http.Request)(*http.Response,error){return response(streamBody,200),nil}),Middleware:[]PoolsterMiddleware{wrapper},ValidateResponses:true})
    request,_=client.newRequest(context.Background(),"GET","/events",nil,nil,nil)
    stream,err:=client.stream(request);if err!=nil{t.Fatal(err)}
    if wrappers!=1||streamBody.closed!=0{t.Fatal("stream closed before consumer read")};stream.Close();if streamBody.closed!=1{t.Fatal("consumer close not forwarded")}
}
func TestPublicConsumerMiddleware(t *testing.T) {
    calls:=0
    policy:=func(next PoolsterHTTPClient) PoolsterHTTPClient{return PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){
        rewritten:=request.Clone(request.Context());rewritten.Header.Set("X-Consumer","acme")
        return next.Do(rewritten)
    })}
    native:=PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){
        calls++;if request.URL.Path!="/v1/contacts/contact_123"||request.Header.Get("X-Consumer")!="acme"||request.Header.Get("Authorization")!="Bearer token"{t.Fatalf("public middleware missed request: %s %#v",request.URL.Path,request.Header)}
        return response(io.NopCloser(strings.NewReader(`{"email":"future","extra":true}`)),200),nil
    })
    client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",APIKey:"token",APIKeyPrefix:"Bearer",HTTPClient:native,Middleware:[]PoolsterMiddleware{policy},ValidateResponses:true});if err!=nil{t.Fatal(err)}
    model,err:=client.GetContact(context.Background(),&GetContactRequest{ContactID:"contact_123"});if err!=nil{t.Fatal(err)}
    if model.Email!="future"||calls!=1||string(model.AdditionalProperties["extra"])!="true"{t.Fatalf("consumer result %#v %d",model,calls)}
}

type failingReader struct { err error }
func(reader failingReader) Read([]byte)(int,error){return 0,reader.err}
func TestValidatedReaderCancellation(t *testing.T){
    for _,cause:=range []error{context.Canceled,context.DeadlineExceeded}{
        body:=&trackedBody{Reader:failingReader{err:cause}};hooks:=&lifecycle{}
        client,_:=NewClient(ClientConfig{BaseURL:"https://example.test",ValidateResponses:true,Hooks:hooks,HTTPClient:PoolsterHTTPClientFunc(func(*http.Request)(*http.Response,error){return response(body,200),nil})})
        _,err:=client.GetContact(context.Background(),&GetContactRequest{ContactID:"contact_123"})
        if !errors.Is(err,cause)||body.closed!=1||hooks.failures!=1{t.Fatalf("reader cancellation lost: %v %#v %d",err,hooks,body.closed)}
    }
}

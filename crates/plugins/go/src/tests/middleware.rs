use super::*;

#[test]
fn author_bundled_middleware_is_registered_without_customer_configuration() {
    use poolster_core::{customization::BundledMiddleware, engine::Packages};
    let middleware = BundledMiddleware { path: "customer.go".into(), contents: "package email\nimport \"net/http\"\nfunc CustomerMiddleware(next PoolsterHTTPClient) PoolsterHTTPClient { return PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response,error) { request.Header.Set(\"X-Bundled\", \"yes\"); return next.Do(request) }) }\n".into(), symbol: "CustomerMiddleware".into(), async_symbol: None };
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("email")
                .with(crate::sdk())
                .middleware(middleware.clone()),
        )
        .generate(&contact_api(), None)
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    fs::write(root.path().join("sdk/defaultmiddleware_test.go"), r#"package email
import ("context"; "io"; "net/http"; "strings"; "testing")
func TestDefaultMiddleware(t *testing.T) {
    transport := PoolsterHTTPClientFunc(func(request *http.Request) (*http.Response,error) {
        if request.Header.Get("X-Bundled") != "yes" { t.Fatal("bundled middleware not registered") }
        return &http.Response{StatusCode:204, Header:http.Header{}, Body:io.NopCloser(strings.NewReader(""))},nil
    })
    client, err := NewClient(ClientConfig{BaseURL:"https://example.test", HTTPClient:transport})
    if err != nil {t.Fatal(err)}
    request,_ := client.newRequest(context.Background(), "GET", "/",nil,nil,nil)
    if err := client.doWithRetry(request,nil); err != nil {t.Fatal(err)}
}
"#).unwrap();
    assert!(
        Command::new("go")
            .args(["test", "./..."])
            .current_dir(root.path().join("sdk"))
            .env("GOCACHE", root.path().join("go-cache"))
            .status()
            .unwrap()
            .success()
    );
    let mut invalid = middleware;
    invalid.contents = "package wrong".into();
    assert!(
        Packages::new()
            .package(
                crate::package("sdk")
                    .name("email")
                    .with(crate::sdk())
                    .middleware(invalid)
            )
            .generate(&contact_api(), None)
            .is_err()
    );
}
#[test]
fn generated_idempotency_keys_are_scoped_fresh_and_retry_stable() {
    let header = OperationParameter {
        name: "X-Key".into(),
        location: "header".into(),
        required: false,
        schema: Some(string_schema()),
        description: None,
        annotations: Default::default(),
    };
    let mut write = Operation {
        id: "write".into(),
        method: HttpMethod::Post,
        path: "/write".into(),
        parameters: vec![header],
        ..Default::default()
    };
    write.annotations.insert(
        "x-poolster-idempotency-resolved".into(),
        serde_json::json!({"header":"X-Key","parameter_name":"X-Key","auto_generate":true}),
    );
    let mut patch = write.clone();
    patch.id = "patch".into();
    patch.method = HttpMethod::Patch;
    let mut unsafe_write = write.clone();
    unsafe_write.id = "unsafeWrite".into();
    unsafe_write.annotations.clear();
    let mut unsafe_patch = unsafe_write.clone();
    unsafe_patch.id = "unsafePatch".into();
    unsafe_patch.method = HttpMethod::Patch;
    let api = Api {
        name: "Keys".into(),
        operations: vec![write, patch, unsafe_write, unsafe_patch],
        ..Default::default()
    };
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&api, "sdk", Some("keys"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/idempotency_test.go"), r#"package keys
import("context"; "io"; "net/http"; "strings"; "testing"; "time"; "regexp")
type keyDriver struct { keys []string; calls int }
func (driver *keyDriver) Do(request *http.Request) (*http.Response,error) {
 driver.calls++;driver.keys=append(driver.keys,request.Header.Get("X-Key"))
 status:=204;if driver.calls%2==1 {status=503}
 return &http.Response{StatusCode:status,Header:http.Header{},Body:io.NopCloser(strings.NewReader(""))},nil
}
func TestBoundedRetryHeaders(t *testing.T) {
 for _,value:=range []string{"", "-1", "NaN", "Infinity", "1e3", ".5", "5."} { if _,ok:=parseRetryAfterMilliseconds(value);ok{t.Fatalf("accepted %q",value)} }
 if duration,ok:=parseRetryAfterMilliseconds(" 1.5 ");!ok||duration!=1500*time.Microsecond{t.Fatal("fractional milliseconds")}
 config:=retryConfig{initialDelay:5*time.Millisecond,maxDelay:50*time.Millisecond}
 response:=&http.Response{Header:http.Header{}}
 response.Header.Set("Retry-After-Ms","25");response.Header.Set("Retry-After","99")
 if config.delay(response,1)!=25*time.Millisecond{t.Fatal("millisecond precedence")}
 response.Header.Set("Retry-After-Ms","10000000000000")
 if config.delay(response,1)!=50*time.Millisecond{t.Fatal("millisecond cap")}
 response.Header.Set("Retry-After-Ms","-1");response.Header.Set("Retry-After",time.Now().Add(time.Hour).UTC().Format(http.TimeFormat))
 if config.delay(response,1)!=50*time.Millisecond{t.Fatal("date cap")}
 response.Header.Set("Retry-After","9223372036854775807")
 if config.delay(response,1)!=50*time.Millisecond{t.Fatal("seconds overflow")}
 response.Header.Set("Retry-After","invalid")
 if config.delay(response,1)!=5*time.Millisecond{t.Fatal("invalid fallback")}
}
func TestScopedKeys(t *testing.T) {
 driver:=&keyDriver{}
 client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:driver,Retry:&RetryConfig{MaxAttempts:2,InitialDelay:time.Nanosecond,MaxDelay:time.Nanosecond}});if err!=nil{t.Fatal(err)}
 if err=client.Write(context.Background(),nil);err!=nil{t.Fatal(err)}
 if len(driver.keys)!=2||driver.keys[0]!=driver.keys[1]{t.Fatal("retry changed key")}
 if !regexp.MustCompile(`^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`).MatchString(driver.keys[0]){t.Fatal("not UUID v4")}
 if err=client.Write(context.Background(),nil);err!=nil{t.Fatal(err)}
 if driver.keys[0]==driver.keys[2]{t.Fatal("key reused across logical calls")}
 caller:="caller";input:=&WriteRequest{XKey:&caller}
 if err=client.Write(context.Background(),input);err!=nil{t.Fatal(err)}
 if driver.keys[4]!="caller"||driver.keys[5]!="caller"||*input.XKey!="caller"{t.Fatal("caller key lost or mutated")}
 if err=client.Patch(context.Background(),nil);err!=nil{t.Fatal(err)}
 before:=driver.calls
 if err=client.UnsafeWrite(context.Background(),&UnsafeWriteRequest{XKey:&caller});err==nil{t.Fatal("expected status error")}
 if driver.calls!=before+1{t.Fatal("unrelated POST retried")}
 driver.calls=0;before=len(driver.keys)
 if err=client.UnsafePatch(context.Background(),nil);err==nil{t.Fatal("expected patch error")}
 if len(driver.keys)!=before+1{t.Fatal("unkeyed PATCH retried")}
 driver.calls=0;before=len(driver.keys);empty:=""
 if err=client.Write(context.Background(),&WriteRequest{XKey:&empty});err==nil{t.Fatal("empty explicit key enabled retry")}
 if len(driver.keys)!=before+1||driver.keys[before]!=""{t.Fatal("empty explicit caller key overwritten")}
 driver.calls=0;before=len(driver.keys);blank:="   "
 if err=client.Write(context.Background(),&WriteRequest{XKey:&blank});err==nil{t.Fatal("blank explicit key enabled retry")}
 if len(driver.keys)!=before+1||driver.keys[before]!="   "{t.Fatal("blank explicit caller key overwritten")}

}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "./..."])
        .current_dir(root.path().join("sdk"))
        .env(
            "GOCACHE",
            std::env::var_os("GOCACHE").unwrap_or_else(|| {
                std::env::temp_dir()
                    .join("poolster-go-cache")
                    .into_os_string()
            }),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn generated_backoff_cancellation_closes_response_and_preserves_cause() {
    let root = tempfile::tempdir().unwrap();
    render_test_sdk(&contact_api(), "sdk", Some("email"))
        .unwrap()
        .write_to(root.path())
        .unwrap();
    fs::write(root.path().join("sdk/cancellation_test.go"), r#"package email
import("context";"errors";"io";"net/http";"testing";"time")
type trackedBody struct {closed bool}
func (body *trackedBody) Read([]byte)(int,error){return 0,io.EOF}
func (body *trackedBody) Close()error{body.closed=true;return nil}
func TestCancelBackoff(t *testing.T){
 ctx,cancel:=context.WithCancel(context.Background());defer cancel();calls:=0;body:=&trackedBody{}
 transport:=PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){calls++;if request.Context()!=ctx{t.Fatal("lost cancellation context")};cancel();return &http.Response{StatusCode:503,Header:http.Header{},Body:body},nil})
 client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:transport,Retry:&RetryConfig{MaxAttempts:3,InitialDelay:time.Hour,MaxDelay:time.Hour}});if err!=nil{t.Fatal(err)}
 done:=make(chan error,1);go func(){_,err:=client.GetContact(ctx,nil);done<-err}()
 select{case err:=<-done:if !errors.Is(err,context.Canceled){t.Fatalf("lost cause: %v",err)};case<-time.After(time.Second):t.Fatal("cancellation did not interrupt retry backoff")}
 if calls!=1||!body.closed{t.Fatal("extra request or unclosed retry response")}
}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-race", "./..."])
        .current_dir(root.path().join("sdk"))
        .env(
            "GOCACHE",
            std::env::var_os("GOCACHE").unwrap_or_else(|| {
                std::env::temp_dir()
                    .join("poolster-go-cache")
                    .into_os_string()
            }),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

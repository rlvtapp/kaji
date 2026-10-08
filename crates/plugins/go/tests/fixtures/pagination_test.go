package email
import("context";"errors";"fmt";"io";"net/http";"strings";"testing")
func pageClient(t *testing.T,expectedLimit string,calls *[]string,reply func(string)string)*Client{
    t.Helper()
    transport:=PoolsterHTTPClientFunc(func(request *http.Request)(*http.Response,error){
        page:=request.URL.Query().Get("page");*calls=append(*calls,page)
        if request.URL.Path!="/contacts"||request.Header.Get("Limit")!=expectedLimit{t.Fatalf("request encoding: %s %#v",request.URL,request.Header)}
        return &http.Response{StatusCode:200,Header:http.Header{},Body:io.NopCloser(strings.NewReader(reply(page)))},nil
    })
    client,err:=NewClient(ClientConfig{BaseURL:"https://example.test",HTTPClient:transport});if err!=nil{t.Fatal(err)};return client
}
func pageBody(items string)string{return `{"groups":[{"a/b~c":[`+items+`]}]}`}
func TestPagePlanStartLimitAndCopies(t *testing.T){
    calls:=[]string{};start,limit:=int64(7),int64(2)
    client:=pageClient(t,"2",&calls,func(page string)string{if page=="7"{return pageBody(`"a","b"`)};return pageBody(`"c"`)})
    input:=&ListContactsRequest{Page:&start,Limit:&limit};pager:=client.ListContactsPages(input)
    limit=3 // Snapshot pointer-backed pagination controls at construction.
    if _,err:=pager.Next(context.Background());err!=nil{t.Fatal(err)}
    if _,err:=pager.Next(context.Background());err!=nil{t.Fatal(err)}
    if _,err:=pager.Next(context.Background());err!=io.EOF{t.Fatal(err)}
    if strings.Join(calls,",")!="7,8"||start!=7||*input.Page!=7||*input.Limit!=3{t.Fatalf("caller inputs changed: %v %#v",calls,input)}
}
func TestPageWithoutLimitStopsAfterEmpty(t *testing.T){
    calls:=[]string{}
    client:=pageClient(t,"",&calls,func(page string)string{if page=="1"{return pageBody(`"a"`)};return pageBody("")})
    pager:=client.ListContactsPages(nil)
    for index:=0;index<2;index++{if _,err:=pager.Next(context.Background());err!=nil{t.Fatal(err)}}
    if _,err:=pager.Next(context.Background());err!=io.EOF||strings.Join(calls,",")!="1,2"{t.Fatalf("termination %v %v",err,calls)}
}
func TestRequiredPageAndFailures(t *testing.T){
    calls:=[]string{};client:=pageClient(t,"2",&calls,func(string)string{return pageBody(`"a"`)})
    pager:=client.ListRequiredPages(&ListRequiredRequest{Page:0,Limit:2})
    if _,err:=pager.Next(context.Background());err!=nil{t.Fatal(err)}
    if _,err:=pager.Next(context.Background());err!=io.EOF||strings.Join(calls,",")!="0"{t.Fatalf("required zero start %v %v",err,calls)}
    zero:=int64(0);bad:=client.ListContactsPages(&ListContactsRequest{Limit:&zero})
    if _,err:=bad.Next(context.Background());err==nil||len(calls)!=1{t.Fatal("nonpositive limit sent")}
    badShape:=pageClient(t,"",&calls,func(string)string{return `{"groups":[]}`})
    if _,err:=badShape.ListContactsPages(nil).Next(context.Background());err==nil{t.Fatal("missing results silently accepted")}
    maximum:=int64(^uint64(0)>>1);overflow:=pageClient(t,"",&calls,func(string)string{return pageBody(`"a"`)}).ListContactsPages(&ListContactsRequest{Page:&maximum})
    if _,err:=overflow.Next(context.Background());err!=nil{t.Fatal(err)}
    before:=len(calls);if _,err:=overflow.Next(context.Background());err==nil||len(calls)!=before{t.Fatal("overflow not bounded")}
    ctx,cancel:=context.WithCancel(context.Background());cancel()
    if _,err:=client.ListContactsPages(nil).Next(ctx);!errors.Is(err,context.Canceled){t.Fatal(err)}
}
func TestPortableSelectorTraversal(t *testing.T){
    value:=map[string]any{"groups":[]any{map[string]any{"next":"first"},map[string]any{"next":"last"}},"a.b":map[string]any{"/key":[]any{1,2}}}
    if text,ok:=poolsterPaginationString(value,"$.groups[-1].next");!ok||text!="last"{t.Fatal(text,ok)}
    if count,ok:=poolsterPaginationArrayLen(value,"/a.b/~1key");!ok||count!=2{t.Fatal(count,ok)}
    for _,selector:=range []string{"$.groups[20].next","$.groups[broken]","/a.b/missing","$.groups[-3]"}{if _,ok:=poolsterPaginationValue(value,selector);ok{t.Fatal(fmt.Sprintf("invalid selector %s accepted",selector))}}
}

package main
import("context";"encoding/json";"fmt";"net/http";"os";sdk "contractsdk")
func main(){
    policy:=func(next sdk.KajiHTTPClient)sdk.KajiHTTPClient{return sdk.KajiHTTPClientFunc(func(request *http.Request)(*http.Response,error){rewritten:=request.Clone(request.Context());rewritten.Header.Set("X-Contract-Middleware","yes");return next.Do(rewritten)})}
    client,err:=sdk.NewClient(sdk.ClientConfig{BaseURL:os.Getenv("KAJI_CONTRACT_URL"),APIKey:os.Getenv("KAJI_CONTRACT_CASE"),APIKeyPrefix:"Bearer",Middleware:[]sdk.KajiMiddleware{policy}})
    if err!=nil{panic(err)}
    model,err:=client.GetContact(context.Background())
    result:=map[string]any{"outcome":"error"}
    if err==nil{result=map[string]any{"outcome":"success","id":model.ID}}
    encoded,_:=json.Marshal(result);fmt.Println(string(encoded))
}

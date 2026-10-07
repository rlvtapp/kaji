package main
import("context";"encoding/json";"fmt";"io";"net/http";"os";"strings";sdk "contractsdk")
func main(){
 var scenario struct{Action string `json:"action"`;CallerKey *string `json:"caller_key"`;Repeats int `json:"repeats"`;Page int64 `json:"page"`;Limit int64 `json:"limit"`;Validate bool `json:"validate_responses"`};if raw:=os.Getenv("KAJI_CONTRACT_SCENARIO");raw!=""{if err:=json.Unmarshal([]byte(raw),&scenario);err!=nil{panic(err)}}
 policy:=func(next sdk.KajiHTTPClient)sdk.KajiHTTPClient{return sdk.KajiHTTPClientFunc(func(request *http.Request)(*http.Response,error){rewritten:=request.Clone(request.Context());rewritten.Header.Set("X-Contract-Middleware","yes");return next.Do(rewritten)})}
 client,err:=sdk.NewClient(sdk.ClientConfig{BaseURL:os.Getenv("KAJI_CONTRACT_URL"),APIKey:os.Getenv("KAJI_CONTRACT_CASE"),APIKeyPrefix:"Bearer",ValidateResponses:scenario.Validate,Middleware:[]sdk.KajiMiddleware{policy}});if err!=nil{panic(err)}
 result:=map[string]any{"outcome":"error"};ctx:=context.Background();var id string
 if scenario.Action=="listContactsPages"{
  ids:=[]string{};pages:=client.ListContactsPages(&sdk.ListContactsRequest{Page:&scenario.Page,Limit:&scenario.Limit});for {var page *sdk.ContactPage;page,err=pages.Next(ctx);if err==io.EOF{err=nil;break};if err!=nil||page==nil{break};for _,item:=range page.Items{ids=append(ids,item.ID)}};id=strings.Join(ids,",")
 }else{
  if scenario.Repeats==0{scenario.Repeats=1};for i:=0;i<scenario.Repeats;i++{
   var model *sdk.Contact
   switch scenario.Action{
   case "echoWire":
    text,label:="héllo 雪","caller";flag:=false;count:=int64(0);tags:=[]string{"a","b"};model,err=client.EchoWire(ctx,&sdk.EchoWireRequest{Key:"café/雪",Text:&text,Flag:&flag,Count:&count,Tags:&tags,XLabel:&label,Body:sdk.WireInput{Enabled:false,Count:0,Note:nil}})
   case "createContact":model,err=client.CreateContact(ctx,&sdk.CreateContactRequest{XOnce:scenario.CallerKey})
   case "patchContact":model,err=client.PatchContact(ctx,&sdk.PatchContactRequest{XOnce:scenario.CallerKey})
   case "unsafeCreateContact":model,err=client.UnsafeCreateContact(ctx)
   case "unsafePatchContact":model,err=client.UnsafePatchContact(ctx)
   default:model,err=client.GetContact(ctx)
   };if err!=nil{break};id=model.ID
  }
 }
 if err==nil{result=map[string]any{"outcome":"success","id":id}}
 encoded,_:=json.Marshal(result);fmt.Println(string(encoded))
}

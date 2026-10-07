package provider
import (
 "context"
 "encoding/json"
 "fmt"
 "io"
 "net/http"
 "strings"
 "testing"
 "github.com/hashicorp/terraform-plugin-framework/attr"
 "github.com/hashicorp/terraform-plugin-framework/resource"
 "github.com/hashicorp/terraform-plugin-framework/datasource"
 "github.com/hashicorp/terraform-plugin-framework/tfsdk"
 "github.com/hashicorp/terraform-plugin-framework/types"
 "github.com/hashicorp/terraform-plugin-go/tftypes"
 "github.com/hashicorp/terraform-plugin-go/tfprotov6"
)
type nestedHTTP struct{body string;mismatch bool}
func(h *nestedHTTP)Do(req *http.Request)(*http.Response,error){
 if req.Method=="POST"||req.Method=="PATCH"{var body map[string]any;if err:=json.NewDecoder(req.Body).Decode(&body);err!=nil{return nil,err};config:=body["config"].(map[string]any);if config["displayName"]!="雪"||config["enabled"]!=false{return nil,fmt.Errorf("wire mapping lost")};config["note"]="server";if h.mismatch{config["displayName"]="changed"};for _,rule:=range body["rules"].([]any){rule.(map[string]any)["note"]="server"};for _,label:=range body["labels"].(map[string]any){label.(map[string]any)["note"]="server"};body["id"]="remote";encoded,_:=json.Marshal(body);h.body=string(encoded)}
 return &http.Response{StatusCode:200,Header:http.Header{},Body:io.NopCloser(strings.NewReader(h.body))},nil
}
func nestedPlan(t *testing.T)ThingResourceModel{
 t.Helper();shape:=ThingResourceConfigShape;object,diags:=types.ObjectValue(shape.attributeTypes(),map[string]attr.Value{"display_name":types.StringValue("雪"),"enabled":types.BoolValue(false),"note":types.StringUnknown()});if diags.HasError(){t.Fatal(diags)}
 rules,d:=types.ListValue(shape.tfType(),[]attr.Value{object});if d.HasError(){t.Fatal(d)};labels,d:=types.MapValue(shape.tfType(),map[string]attr.Value{"x":object});if d.HasError(){t.Fatal(d)}
 tags,d:=types.ListValue(types.StringType,[]attr.Value{types.StringValue("one"),types.StringValue("two")});if d.HasError(){t.Fatal(d)};properties,d:=types.MapValue(types.StringType,map[string]attr.Value{"empty":types.StringValue("")});if d.HasError(){t.Fatal(d)}
 return ThingResourceModel{ID:types.StringUnknown(),Config:object,Rules:rules,Labels:labels,Tags:tags,Properties:properties}
}
func TestNestedLifecycle(t *testing.T){
 ctx:=context.Background();driver:=&nestedHTTP{};r:=&ThingResource{client:&apiClient{baseURL:"https://example.test",httpClient:driver}};schemaResponse:=resource.SchemaResponse{};r.Schema(ctx,resource.SchemaRequest{},&schemaResponse);if diags:=schemaResponse.Schema.ValidateImplementation(ctx);diags.HasError(){t.Fatal(diags)}
 plan:=tfsdk.Plan{Schema:schemaResponse.Schema,Raw:tftypes.NewValue(schemaResponse.Schema.Type().TerraformType(ctx),nil)};model:=nestedPlan(t);if d:=plan.Set(ctx,&model);d.HasError(){t.Fatal(d)}
 created:=resource.CreateResponse{State:tfsdk.State{Schema:schemaResponse.Schema,Raw:tftypes.NewValue(schemaResponse.Schema.Type().TerraformType(ctx),nil)}};r.Create(ctx,resource.CreateRequest{Plan:plan},&created);if created.Diagnostics.HasError(){t.Fatal(created.Diagnostics)};if !created.State.Raw.IsFullyKnown(){t.Fatal("unknown state after create")};var state ThingResourceModel;if d:=created.State.Get(ctx,&state);d.HasError(){t.Fatal(d)};if state.ID.ValueString()!="remote"||state.Config.Attributes()["note"].(types.String).ValueString()!="server"{t.Fatal("nested hydration lost")}
 updated:=resource.UpdateResponse{State:created.State};r.Update(ctx,resource.UpdateRequest{Plan:plan,State:created.State},&updated);if updated.Diagnostics.HasError(){t.Fatal(updated.Diagnostics)};read:=resource.ReadResponse{State:updated.State};r.Read(ctx,resource.ReadRequest{State:updated.State},&read);if read.Diagnostics.HasError(){t.Fatal(read.Diagnostics)}
 driver.mismatch=true;failed:=resource.CreateResponse{State:tfsdk.State{Schema:schemaResponse.Schema,Raw:tftypes.NewValue(schemaResponse.Schema.Type().TerraformType(ctx),nil)}};r.Create(ctx,resource.CreateRequest{Plan:plan},&failed);if !failed.Diagnostics.HasError(){t.Fatal("accepted changed configured nested field")};if !failed.State.Raw.IsFullyKnown(){t.Fatal("recoverable state has unknown values")};if d:=failed.State.Get(ctx,&state);d.HasError(){t.Fatal(d)};if state.ID.ValueString()!="remote"{t.Fatal("created identity lost on invalid response")}
}
func TestNestedNullUnknownAndWireValidation(t *testing.T){
 shape:=ThingResourceConfigShape;for _,raw:=range []string{"null","{\"displayName\":\"x\"}","{\"displayName\":\"x\",\"enabled\":null}","{\"displayName\":2,\"enabled\":true}"}{if _,err:=shape.decode([]byte(raw));err==nil{t.Fatal("accepted invalid wire object")}}
 object:=nestedPlan(t).Config;values:=object.Attributes();values["display_name"]=types.StringUnknown();unknown,_:=types.ObjectValue(shape.attributeTypes(),values);if _,err:=shape.encode(unknown);err==nil{t.Fatal("encoded required unknown")};if !finalizeNested(shape,unknown).(types.Object).Attributes()["display_name"].IsNull(){t.Fatal("did not finalize unknown")}
 list:=ThingResourceTagsShape;empty,_:=types.ListValue(types.StringType,[]attr.Value{});wire,err:=list.encode(empty);if err!=nil{t.Fatal(err)};encoded,_:=json.Marshal(wire);if string(encoded)!="[]"{t.Fatal("empty list became null")};decoded,err:=list.decode(encoded);if err!=nil||!decoded.Equal(empty){t.Fatal("empty list did not roundtrip")}
}

func TestStateUpgradeExecutesAndRejectsDataLoss(t *testing.T){
 ctx:=context.Background();r:=&ThingResource{};schemaResponse:=resource.SchemaResponse{};r.Schema(ctx,resource.SchemaRequest{},&schemaResponse);if diags:=schemaResponse.Schema.ValidateImplementation(ctx);diags.HasError(){t.Fatal(diags)};if schemaResponse.Schema.Version!=1{t.Fatal("version not exposed")};upgrader:=r.UpgradeState(ctx)[0];
 raw:=map[string]any{"id":"remote","old_config":map[string]any{"display_name":"雪","enabled":false,"note":nil},"rules":[]any{},"labels":map[string]any{},"tags":[]any{},"properties":map[string]any{}}
 encoded,_:=json.Marshal(raw);response:=resource.UpgradeStateResponse{};upgrader.StateUpgrader(ctx,resource.UpgradeStateRequest{RawState:&tfprotov6.RawState{JSON:encoded}},&response);if response.Diagnostics.HasError()||response.DynamicValue==nil{t.Fatal(response.Diagnostics)};value,err:=response.DynamicValue.Unmarshal(schemaResponse.Schema.Type().TerraformType(ctx));if err!=nil||!value.IsFullyKnown(){t.Fatal("migration did not produce valid state",err)}
 state:=tfsdk.State{Schema:schemaResponse.Schema,Raw:value};var model ThingResourceModel;if d:=state.Get(ctx,&model);d.HasError(){t.Fatal(d)};if model.ID.ValueString()!="remote"||model.Config.Attributes()["display_name"].(types.String).ValueString()!="雪"||model.Config.Attributes()["enabled"].(types.Bool).ValueBool()!=false||!model.Config.Attributes()["note"].IsNull(){t.Fatal("migration lost nested data")}
 for _,mode:=range []string{"collision","unknown","type"}{bad:=map[string]any{};for k,v:=range raw{bad[k]=v};switch mode{case "collision":bad["config"]=bad["old_config"];case "unknown":bad["unrelated"]="secret";case "type":bad["old_config"]="wrong"};encoded,_:=json.Marshal(bad);response=resource.UpgradeStateResponse{};upgrader.StateUpgrader(ctx,resource.UpgradeStateRequest{RawState:&tfprotov6.RawState{JSON:encoded}},&response);if !response.Diagnostics.HasError()||response.DynamicValue!=nil{t.Fatal("accepted lossy migration",mode)}}
}

func TestNestedDataSourceRead(t *testing.T){
 ctx:=context.Background();model:=nestedPlan(t);model.ID=types.StringValue("remote");body:=map[string]any{"id":"remote"};for name,shape:=range map[string]wireShape{"config":ThingResourceConfigShape,"rules":ThingResourceRulesShape,"labels":ThingResourceLabelsShape,"tags":ThingResourceTagsShape,"properties":ThingResourcePropertiesShape}{var value attr.Value;switch name{case "config":value=model.Config;case "rules":value=model.Rules;case "labels":value=model.Labels;case "tags":value=model.Tags;default:value=model.Properties};wire,err:=shape.encode(value);if err!=nil{t.Fatal(err)};body[name]=wire};encoded,_:=json.Marshal(body);d:=&ThingDataSource{client:&apiClient{baseURL:"https://example.test",httpClient:&nestedHTTP{body:string(encoded)}}};schemaResponse:=datasource.SchemaResponse{};d.Schema(ctx,datasource.SchemaRequest{},&schemaResponse);if diags:=schemaResponse.Schema.ValidateImplementation(ctx);diags.HasError(){t.Fatal(diags)};schema:=schemaResponse.Schema
 model.Config=types.ObjectNull(ThingResourceConfigShape.attributeTypes());model.Rules=types.ListNull(ThingResourceConfigShape.tfType());model.Labels=types.MapNull(ThingResourceConfigShape.tfType());model.Tags=types.ListNull(types.StringType);model.Properties=types.MapNull(types.StringType);state:=tfsdk.State{Schema:schema,Raw:tftypes.NewValue(schema.Type().TerraformType(ctx),nil)};if diag:=state.Set(ctx,&model);diag.HasError(){t.Fatal(diag)};response:=datasource.ReadResponse{State:state};d.Read(ctx,datasource.ReadRequest{Config:tfsdk.Config{Schema:schema,Raw:state.Raw}},&response);if response.Diagnostics.HasError()||!response.State.Raw.IsFullyKnown(){t.Fatal(response.Diagnostics)};if diag:=response.State.Get(ctx,&model);diag.HasError(){t.Fatal(diag)};if model.Config.Attributes()["display_name"].(types.String).ValueString()!="雪"||!model.Config.Attributes()["note"].IsNull(){t.Fatal("data source lost nested fields")}
}
func TestNestedCollectionConsistencyAndIntegerPrecision(t *testing.T){
 model:=nestedPlan(t);empty,_:=types.ListValue(ThingResourceConfigShape.tfType(),[]attr.Value{});if _,err:=mergeNested(ThingResourceRulesShape,model.Rules,empty,false);err==nil{t.Fatal("accepted changed known list length")};other,_:=types.MapValue(ThingResourceConfigShape.tfType(),map[string]attr.Value{"changed":model.Config});if _,err:=mergeNested(ThingResourceLabelsShape,model.Labels,other,false);err==nil{t.Fatal("accepted changed known map keys")};shape:=wireShape{Kind:"scalar",Ty:"int64"};value,err:=shape.decode([]byte("9223372036854775807"));if err!=nil||value.(types.Int64).ValueInt64()!=9223372036854775807{t.Fatal("integer precision lost")};wire,err:=shape.encode(value);if err!=nil{t.Fatal(err)};encoded,_:=json.Marshal(wire);if string(encoded)!="9223372036854775807"{t.Fatal("integer serialization lost")}
}

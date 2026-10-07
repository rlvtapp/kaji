use crate::plan::ResourcePlan;
use std::fmt::Write;

pub(crate) fn source(plan: &ResourcePlan, ty: &str) -> String {
    let mut output = format!(
        "\nvar _ resource.ResourceWithUpgradeState = &{ty}{{}}\nfunc(r *{ty})UpgradeState(_ context.Context)map[int64]resource.StateUpgrader{{return map[int64]resource.StateUpgrader{{\n"
    );
    for upgrade in &plan.state_upgrades {
        let _ = writeln!(
            output,
            "{}:{{StateUpgrader:func(ctx context.Context,req resource.UpgradeStateRequest,resp *resource.UpgradeStateResponse){{",
            upgrade.version
        );
        output.push_str("if req.RawState==nil||len(req.RawState.JSON)==0||len(req.RawState.JSON)>10<<20{resp.Diagnostics.AddError(\"Invalid prior state\",\"Upgrade requires bounded JSON state.\");return};var values map[string]json.RawMessage;if json.Unmarshal(req.RawState.JSON,&values)!=nil||values==nil{resp.Diagnostics.AddError(\"Invalid prior state\",\"Prior state must be a JSON object.\");return}\n");
        for (from, to) in &upgrade.rename_fields {
            let from = serde_json::to_string(from).unwrap();
            let to = serde_json::to_string(to).unwrap();
            let _ = writeln!(
                output,
                "{{value,present:=values[{from}];if !present{{resp.Diagnostics.AddError(\"State upgrade failed\",\"Prior attribute missing.\");return}};if _,collision:=values[{to}];collision{{resp.Diagnostics.AddError(\"State upgrade failed\",\"Rename target already exists.\");return}};values[{to}]=value;delete(values,{from})}}"
            );
        }
        output.push_str("schemaResponse:=resource.SchemaResponse{};r.Schema(ctx,resource.SchemaRequest{},&schemaResponse);resp.Diagnostics.Append(schemaResponse.Diagnostics...);if resp.Diagnostics.HasError(){return};if len(values)!=len(schemaResponse.Schema.Attributes){resp.Diagnostics.AddError(\"State upgrade failed\",\"Prior state contains unmatched or missing attributes.\");return};for name:=range schemaResponse.Schema.Attributes{if _,ok:=values[name];!ok{resp.Diagnostics.AddError(\"State upgrade failed\",\"Prior state does not match current attributes.\");return}};encoded,err:=json.Marshal(values);if err!=nil{resp.Diagnostics.AddError(\"State upgrade failed\",\"Cannot encode state.\");return};currentType:=schemaResponse.Schema.Type().TerraformType(ctx);state,err:=tftypes.ValueFromJSON(encoded,currentType);if err!=nil{resp.Diagnostics.AddError(\"State upgrade failed\",\"Attribute types are incompatible with the current schema.\");return};dynamic,err:=tfprotov6.NewDynamicValue(currentType,state);if err!=nil{resp.Diagnostics.AddError(\"State upgrade failed\",\"Cannot encode current state.\");return};resp.DynamicValue=&dynamic\n}},\n");
    }
    output.push_str("}}\n");
    output
}

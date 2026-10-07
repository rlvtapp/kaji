use crate::plan::{PollCriterion, PollingBinding, ResourcePlan};
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn config(binding: &PollingBinding) -> String {
    let criteria = |items: &[PollCriterion]| {
        items
            .iter()
            .map(|item| match item {
                PollCriterion::Status { status } => format!("{{status:{status}}}"),
                PollCriterion::Body { pointer, equals } => format!(
                    "{{pointer:{},expected:{},body:true}}",
                    quote(pointer),
                    quote(&serde_json::to_string(equals).unwrap())
                ),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "pollingConfig{{delayMS:{},intervalMS:{},maxAttempts:{},success:[]pollingCriterion{{{}}},failure:[]pollingCriterion{{{}}}}}",
        binding.delay_ms,
        binding.interval_ms,
        binding.max_attempts,
        criteria(&binding.success),
        criteria(&binding.failure)
    )
}
fn read_route(plan: &ResourcePlan, ty: &str, id: &str) -> String {
    if plan.identity.is_empty() {
        format!(
            "readRoute:=strings.Replace({}, {},identityPath({id}),1);",
            quote(&plan.read.path),
            quote(&format!("{{{}}}", plan.id_parameter))
        )
    } else {
        format!(
            "readRoute,routeErr:={ty}Route({}, {id});if routeErr!=nil{{resp.Diagnostics.AddError(\"Invalid composite identity\",routeErr.Error());return}};",
            quote(&plan.read.path)
        )
    }
}
pub(crate) fn apply(output: String, plan: &ResourcePlan, ty: &str) -> String {
    let Some(polling) = &plan.polling else {
        return output;
    };
    let mut output = output.replace(" \"context\"\n", " \"context\"\n \"time\"\n");
    let mut lines = Vec::new();
    for line in output.lines() {
        let mut line = line.to_owned();
        for (method, binding) in [
            ("Create", &polling.create),
            ("Update", &polling.update),
            ("Delete", &polling.delete),
        ] {
            let Some(binding) = binding else {
                continue;
            };
            if !line.starts_with(&format!("func(r *{ty}){method}(")) {
                continue;
            }
            let prefix = line.find("{var data").expect("generated lifecycle model");
            line.insert_str(prefix+1,&format!("ctx,cancel:=context.WithTimeout(ctx,time.Duration({})*time.Millisecond);defer cancel();",binding.timeout_ms));
            // Only the mutation is opted into accepting HTTP 202; ordinary Read stays synchronous.
            line = line.replacen("r.client.call(ctx,", "r.client.callPolling(ctx,", 1);
            if method == "Create" {
                let marker = "data.ID=types.StringValue(id);";
                let wait = format!(
                    "{marker}recovery:=data;recovery.finalizeUnknowns();resp.Diagnostics.Append(resp.State.Set(ctx,&recovery)...);if resp.Diagnostics.HasError(){{return}};{}body,err=r.client.waitPolling(ctx,readRoute,{}, {},true,false);if err!=nil{{resp.Diagnostics.AddError(\"Create polling failed\",err.Error());return}};finalID,identityErr:={ty}Identity(body);if identityErr!=nil||finalID!=id{{resp.Diagnostics.AddError(\"Invalid create identity\",\"Completed resource must retain the managed identity.\");return}};",
                    read_route(plan, ty, "data.ID.ValueString()"),
                    plan.requires_auth,
                    config(binding)
                );
                line = line.replace(marker, &wait).replace(
                    "data.hydrate(body,false,false)",
                    "data.hydrate(body,false,true)",
                );
            } else if method == "Update" {
                let old = format!(
                    "body,err:=r.client.call(ctx,{},readRoute,nil,{})",
                    quote(plan.read.method.as_str()),
                    plan.requires_auth
                );
                line = line.replace(
                    &old,
                    &format!(
                        "body,err:=r.client.waitPolling(ctx,readRoute,{}, {},false,false)",
                        plan.requires_auth,
                        config(binding)
                    ),
                );
            } else {
                let ending = ";resp.State.RemoveResource(ctx)}";
                let wait = format!(
                    ";{}_,err=r.client.waitPolling(ctx,readRoute,{}, {},false,true);if err!=nil{{resp.Diagnostics.AddError(\"Delete polling failed\",err.Error());return}};resp.State.RemoveResource(ctx)}}",
                    read_route(plan, ty, "data.ID.ValueString()"),
                    plan.requires_auth,
                    config(binding)
                );
                assert!(line.ends_with(ending), "generated delete ending");
                line = line.strip_suffix(ending).unwrap().to_owned() + &wait;
            }
        }
        lines.push(line);
    }
    output = lines.join("\n") + "\n";
    output
}
pub(crate) const TRANSPORT: &str = r#"
func(c *apiClient)pollingStatus(ctx context.Context,method,route string,body []byte,secure bool)([]byte,int,error){
 if err:=ctx.Err();err!=nil{return nil,0,err}
 request,err:=http.NewRequestWithContext(ctx,method,c.baseURL+route,bytes.NewReader(body));if err!=nil{return nil,0,fmt.Errorf("Invalid API request")}
 request.Header.Set("Accept","application/json");if len(body)>0{request.Header.Set("Content-Type","application/json")}
 __AUTH__
 response,err:=c.httpClient.Do(request);if err!=nil{if ctx.Err()!=nil{return nil,0,ctx.Err()};return nil,0,fmt.Errorf("API transport failed")};defer response.Body.Close()
 data,err:=io.ReadAll(io.LimitReader(response.Body,(10<<20)+1));if err!=nil{if ctx.Err()!=nil{return nil,0,ctx.Err()};return nil,0,fmt.Errorf("API response body could not be read")};if len(data)>10<<20{return nil,0,fmt.Errorf("API response exceeds 10 MiB")}
 return data,response.StatusCode,nil
}
func(c *apiClient)callPolling(ctx context.Context,method,route string,body []byte,secure bool)([]byte,error){
 data,status,err:=c.pollingStatus(ctx,method,route,body,secure);if err!=nil{return nil,err};if status<200||status>=300{return nil,&apiStatusError{status:status}}
 if len(bytes.TrimSpace(data))>0{if _,err:=pollingJSON(data);err!=nil{return nil,err}}
 return data,nil
}
"#;

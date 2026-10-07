use crate::plan::ResourcePlan;
use std::fmt::Write;
fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

pub(crate) fn apply(
    mut output: String,
    plan: &ResourcePlan,
    ty: &str,
    data_source: bool,
) -> String {
    if plan.identity.is_empty() {
        return output;
    }
    let template = quote(&format!("{{{}}}", plan.id_parameter));
    for route in [&plan.read.path, &plan.delete.path]
        .into_iter()
        .chain(plan.update.as_ref().map(|op| &op.path))
    {
        for id in [
            "data.ID.ValueString()",
            "prior.ID.ValueString()",
            "expected",
        ] {
            for spacing in [", ", ","] {
                let original = format!(
                    "strings.Replace({}{}{template},identityPath({id}),1)",
                    quote(route),
                    spacing
                );
                output = output.replace(&format!("route:={original};"), &format!("route,routeErr:={ty}Route({}, {id});if routeErr!=nil{{resp.Diagnostics.AddError(\"Invalid composite identity\",routeErr.Error());return}};",quote(route)));
                output = output.replace(&format!("readRoute:={original};"), &format!("readRoute,routeErr:={ty}Route({}, {id});if routeErr!=nil{{resp.Diagnostics.AddError(\"Invalid composite identity\",routeErr.Error());return}};",quote(route)));
            }
        }
    }
    if data_source {
        return output.replace(";\"strings\"", "");
    }
    if !plan.create.parameters.is_empty() {
        let mut create = format!("createRoute:={};", quote(&plan.create.path));
        for parameter in &plan.create.parameters {
            let binding = plan
                .identity
                .iter()
                .find(|id| id.parameter == parameter.name)
                .expect("validated parent mapping");
            let attribute = plan
                .attributes
                .iter()
                .find(|a| a.wire_name == binding.field)
                .expect("validated parent attribute");
            let field = crate::typed_render::field(&attribute.name);
            let _ = write!(
                create,
                "if data.{field}.IsNull()||data.{field}.IsUnknown()||data.{field}.ValueString()==\"\"{{resp.Diagnostics.AddError(\"Invalid parent identity\",\"Parent identity must be a known nonempty string.\");return}};createRoute=strings.Replace(createRoute,{},identityPath(data.{field}.ValueString()),1);",
                quote(&format!("{{{}}}", parameter.name))
            );
        }
        let original = format!(
            "body,err:=r.client.call(ctx,{},{},payload,",
            quote(plan.create.method.as_str()),
            quote(&plan.create.path)
        );
        output = output.replace(
            &original,
            &format!(
                "{create}body,err:=r.client.call(ctx,{},createRoute,payload,",
                quote(plan.create.method.as_str())
            ),
        );
    }
    // These are exact generator-owned definitions; supplied source is never rewritten.
    let start = output
        .find(&format!("func {ty}Identity("))
        .expect("generated identity");
    let end = output[start..].find('\n').unwrap() + start;
    let mut identity = format!(
        "func {ty}Identity(body []byte)(string,error){{var values map[string]json.RawMessage;if json.Unmarshal(body,&values)!=nil{{return \"\",fmt.Errorf(\"Invalid response identity\")}};parts:=map[string]string{{}};"
    );
    for binding in &plan.identity {
        let _ = write!(
            identity,
            "{{var part string;if json.Unmarshal(values[{}],&part)!=nil||part==\"\"{{return \"\",fmt.Errorf(\"Response requires nonempty composite identity components\")}};parts[{}]=part}};",
            quote(&binding.field),
            quote(&binding.parameter)
        );
    }
    identity.push_str("encoded,_:=json.Marshal(parts);if len(encoded)>8192{return \"\",fmt.Errorf(\"Composite identity exceeds bound\")};return string(encoded),nil}");
    output.replace_range(start..end, &identity);
    if !plan.create.parameters.is_empty() {
        output = output.replacen(
            &format!("id,err:={ty}Identity(body);"),
            "id,err:=data.managedIdentity(body);",
            1,
        );
        let _ = write!(
            output,
            "\nfunc(data *{ty}Model)managedIdentity(body []byte)(string,error){{var values map[string]json.RawMessage;if json.Unmarshal(body,&values)!=nil{{return \"\",fmt.Errorf(\"Invalid response identity\")}};parts:=map[string]string{{}};"
        );
        for binding in &plan.identity {
            if !plan
                .create
                .parameters
                .iter()
                .any(|p| p.name == binding.parameter)
            {
                let _ = write!(
                    output,
                    "{{var part string;if json.Unmarshal(values[{}],&part)!=nil||part==\"\"{{return \"\",fmt.Errorf(\"Response requires nonempty child identity\")}};parts[{}]=part}};",
                    quote(&binding.field),
                    quote(&binding.parameter)
                );
            }
        }
        for parameter in &plan.create.parameters {
            let binding = plan
                .identity
                .iter()
                .find(|id| id.parameter == parameter.name)
                .unwrap();
            let attribute = plan
                .attributes
                .iter()
                .find(|a| a.wire_name == binding.field)
                .unwrap();
            let field = crate::typed_render::field(&attribute.name);
            let _ = write!(
                output,
                "parts[{}]=data.{field}.ValueString();",
                quote(&binding.parameter)
            );
        }
        output.push_str("encoded,_:=json.Marshal(parts);if len(encoded)>8192{return \"\",fmt.Errorf(\"Composite identity exceeds bound\")};return string(encoded),nil}\n");
    }
    let start = output
        .find(&format!("func(r *{ty})ImportState("))
        .expect("generated import");
    let end = output[start..].find('\n').unwrap() + start;
    output.replace_range(start..end,&format!("func(r *{ty})ImportState(ctx context.Context,req resource.ImportStateRequest,resp *resource.ImportStateResponse){{parts,err:={ty}ParseIdentity(req.ID);if err!=nil{{resp.Diagnostics.AddError(\"Invalid import identity\",err.Error());return}};encoded,_:=json.Marshal(parts);req.ID=string(encoded);resource.ImportStatePassthroughID(ctx,path.Root(\"id\"),req,resp)}}"));
    let keys = plan
        .identity
        .iter()
        .map(|binding| quote(&binding.parameter))
        .collect::<Vec<_>>()
        .join(",");
    let _ = writeln!(
        output,
        "func {ty}ParseIdentity(id string)(map[string]string,error){{if len(id)>8192{{return nil,fmt.Errorf(\"Composite identity exceeds bound\")}};decoder:=json.NewDecoder(strings.NewReader(id));token,err:=decoder.Token();if err!=nil||token!=json.Delim('{{'){{return nil,fmt.Errorf(\"Composite identity must be a JSON object\")}};parts:=map[string]string{{}};for decoder.More(){{key,err:=decoder.Token();if err!=nil{{return nil,fmt.Errorf(\"Invalid composite identity\")}};name,ok:=key.(string);if !ok{{return nil,fmt.Errorf(\"Invalid composite identity\")}};if _,duplicate:=parts[name];duplicate{{return nil,fmt.Errorf(\"Duplicate composite identity component\")}};var value string;if decoder.Decode(&value)!=nil||value==\"\"{{return nil,fmt.Errorf(\"Composite identity components must be nonempty strings\")}};parts[name]=value}};if _,err:=decoder.Token();err!=nil{{return nil,fmt.Errorf(\"Invalid composite identity\")}};var extra any;if err:=decoder.Decode(&extra);err!=io.EOF{{return nil,fmt.Errorf(\"Invalid trailing composite identity\")}};keys:=[]string{{{keys}}};if len(parts)!=len(keys){{return nil,fmt.Errorf(\"Composite identity component mismatch\")}};for _,key:=range keys{{if _,ok:=parts[key];!ok{{return nil,fmt.Errorf(\"Composite identity component missing\")}}}};return parts,nil}}"
    );
    let _ = writeln!(
        output,
        "func {ty}Route(route,id string)(string,error){{parts,err:={ty}ParseIdentity(id);if err!=nil{{return \"\",err}};"
    );
    for binding in &plan.identity {
        let _ = writeln!(
            output,
            "route=strings.Replace(route,{},identityPath(parts[{}]),1);",
            quote(&format!("{{{}}}", binding.parameter)),
            quote(&binding.parameter)
        );
    }
    output.push_str("return route,nil}\n");
    output.replace(" \"context\"\n", " \"context\"\n \"io\"\n")
}

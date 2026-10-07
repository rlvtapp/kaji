//! Declared page-number pagination using the neutral validated plan.
use super::*;
use kaji_core::pagination::{PaginationPlan, normalize_pagination};
pub(crate) fn plan(api: &Api, op: &Operation) -> anyhow::Result<Option<PaginationPlan>> {
    let Some(raw) = op
        .annotations
        .get("x-kaji-pagination")
        .or_else(|| op.annotations.get("x-speakeasy-pagination"))
    else {
        return Ok(None);
    };
    anyhow::ensure!(
        raw.get("type").and_then(serde_json::Value::as_str) == Some("page"),
        "Ruby currently supports declared page pagination only"
    );
    let plan = normalize_pagination(api, op, None)?;
    if let Some(plan) = &plan {
        if plan
            .inputs
            .iter()
            .any(|input| input.location == "requestBody")
        {
            anyhow::ensure!(
                op.request_body.as_ref().is_some_and(|body| body.required),
                "body pagination requires a required JSON body"
            );
        }
    }
    Ok(plan)
}
pub(crate) fn render(api: &Api, op: &Operation) -> Option<String> {
    let plan = plan(api, op).ok()??;
    let page = plan.inputs.iter().find(|input| input.role == "page")?;
    let limit = plan.inputs.iter().find(|input| input.role == "limit");
    let selector = &plan.results?.expression;
    let name = ruby_identifier(&snake_case(&op.id));
    let mut args = op
        .parameters
        .iter()
        .map(|p| {
            let id = ruby_identifier(&p.name);
            if p.required {
                format!("{id}:")
            } else {
                format!("{id}: nil")
            }
        })
        .collect::<Vec<_>>();
    if let Some(body) = &op.request_body {
        args.push(if body.required { "body:" } else { "body: nil" }.into())
    }
    let page_arg = ruby_identifier(&page.name);
    let state = if page.location == "requestBody" {
        format!(
            "      kaji_page = kaji_json_path(body, {})\n      kaji_page = 1 if kaji_page.nil?\n      kaji_body = kaji_with_body_value(body, {}, kaji_page)\n",
            ruby_string(&format!(
                "/{}",
                page.name.replace('~', "~0").replace('/', "~1")
            )),
            ruby_string(&page.name)
        )
    } else {
        format!("      kaji_page = {page_arg}.nil? ? 1 : {page_arg}\n")
    };
    let mut forwarded = op
        .parameters
        .iter()
        .map(|p| {
            let id = ruby_identifier(&p.name);
            format!(
                "{id}: {}",
                if page.location != "requestBody" && p.name == page.name {
                    "kaji_page"
                } else {
                    &id
                }
            )
        })
        .collect::<Vec<_>>();
    if op.request_body.is_some() {
        forwarded.push(
            if page.location == "requestBody" {
                "body: kaji_body"
            } else {
                "body: body"
            }
            .into(),
        )
    }
    let limit = limit
        .map(|input| {
            if input.location == "requestBody" {
                format!(
                    "kaji_json_path(kaji_body, {})",
                    ruby_string(&format!(
                        "/{}",
                        input.name.replace('~', "~0").replace('/', "~1")
                    ))
                )
            } else {
                ruby_identifier(&input.name)
            }
        })
        .unwrap_or("nil".into());
    let update = if page.location == "requestBody" {
        format!(
            "        kaji_body = kaji_with_body_value(kaji_body, {}, kaji_page)\n",
            ruby_string(&page.name)
        )
    } else {
        String::new()
    };
    Some(format!(
        "    def {name}_pages({signature})\n      return Enumerator.new {{ |output| {name}_pages({original}) {{ |value| output << value }} }} unless block_given?\n{state}      raise ArgumentError, 'page must be a nonnegative integer' unless kaji_page.is_a?(Integer) && kaji_page >= 0\n      10_000.times do\n        response = {name}({forwarded})\n        items = kaji_json_path(response, {selector})\n        raise TypeError, 'pagination results must be an array' unless items.is_a?(Array)\n        yield response\n        kaji_limit = {limit}\n        return if items.empty? || (kaji_limit.is_a?(Integer) && kaji_limit > 0 && items.length < kaji_limit)\n        kaji_page += 1\n{update}      end\n      raise RuntimeError, 'pagination exceeded 10000 pages'\n    end\n\n",
        signature = args.join(", "),
        original = op
            .parameters
            .iter()
            .map(|p| {
                let id = ruby_identifier(&p.name);
                format!("{id}: {id}")
            })
            .chain(op.request_body.as_ref().map(|_| "body: body".into()))
            .collect::<Vec<_>>()
            .join(", "),
        forwarded = forwarded.join(", "),
        selector = ruby_string(selector)
    ))
}
pub(crate) const HELPERS: &str = r#"    def kaji_json_path(value, path)
      current = value
      if path.start_with?('/')
        return nil if path.match?(/~(?![01])/)
        segments = path[1..-1].split('/', -1).map { |part| part.gsub('~1', '/').gsub('~0', '~') }
      elsif path.start_with?('$')
        segments = []; rest = path[1..-1]
        until rest.empty?
          match = /\A(?:\.([^.\[\]]+)|\[(-?\d+)\])/.match(rest)
          return nil unless match
          segments << (match[1] || match[2].to_i)
          rest = rest[match[0].length..-1]
        end
      else
        return nil
      end
      segments.each do |segment|
        current = current.to_h if !current.is_a?(Hash) && !current.is_a?(Array) && current.respond_to?(:to_h)
        if current.is_a?(Array)
          return nil if path.start_with?('/') && !segment.match?(/\A(?:0|[1-9][0-9]*)\z/)
          return nil unless path.start_with?('/') || segment.is_a?(Integer)
          index = segment.to_i
          return nil unless index >= -current.length && index < current.length
          current = current[index]
        elsif current.is_a?(Hash)
          return nil unless current.key?(segment) || current.key?(segment.to_s.to_sym)
          current = current.key?(segment) ? current[segment] : current[segment.to_s.to_sym]
        else
          return nil
        end
      end
      current
    end

    def kaji_with_body_value(body, key, value)
      data = body.is_a?(Hash) ? body : body.to_h
      raise TypeError, 'body pagination requires an object JSON body' unless data.is_a?(Hash)
      copy = data.each_with_object({}) { |(key, item), result| result[key.to_s] = item }
      copy[key] = value
      body.is_a?(Hash) ? copy : body.class.from_hash(copy)
    end

"#;

#[cfg(test)]
mod tests {
    use super::*;
    use kaji_core::{
        Field, HttpMethod, OperationParameter, OperationRequestBody, OperationResponse,
    };
    fn fixture(body: bool) -> Api {
        let mut api = Api {
            name: "Paging".into(),
            ..Default::default()
        };
        let object = |fields| {
            SchemaValue::new(SchemaKind::Object {
                fields,
                additional_properties: AdditionalProperties::Forbidden,
            })
        };
        let field = |name: &str, kind, required| Field {
            name: name.into(),
            value: SchemaValue::new(kind),
            required,
            annotations: Default::default(),
        };
        let mut op = Operation {
            id: if body { "searchItems" } else { "listItems" }.into(),
            method: if body {
                HttpMethod::Post
            } else {
                HttpMethod::Get
            },
            path: if body { "/search" } else { "/items" }.into(),
            responses: vec![OperationResponse::json(
                "200",
                object(vec![field(
                    "items",
                    SchemaKind::Array {
                        items: Box::new(SchemaValue::new(SchemaKind::Integer)),
                    },
                    true,
                )]),
            )],
            ..Default::default()
        };
        op.parameters = if body {
            vec![OperationParameter {
                name: "page".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            }]
        } else {
            ["page", "limit"]
                .iter()
                .map(|name| OperationParameter {
                    name: (*name).into(),
                    location: "query".into(),
                    required: false,
                    schema: Some(SchemaValue::new(SchemaKind::Integer)),
                    description: None,
                    annotations: Default::default(),
                })
                .collect()
        };
        if body {
            api.schemas.push(Schema::new(
                "PageInput",
                object(vec![
                    field("page", SchemaKind::Integer, false),
                    field("limit", SchemaKind::Integer, true),
                    field("filter", SchemaKind::String, true),
                ]),
            ));
            op.request_body = Some(OperationRequestBody::json(
                SchemaValue::reference("#/components/schemas/PageInput"),
                true,
            ));
        }
        op.annotations.insert("x-kaji-pagination".into(),serde_json::json!({"type":"page","inputs":[{"name":"page","in":if body{"requestBody"}else{"parameters"},"type":"page"},{"name":"limit","in":if body{"requestBody"}else{"parameters"},"type":"limit"}],"outputs":{"results":"/items"}}));
        api.operations.push(op);
        api
    }
    #[test]
    fn page_enumerators_execute_native_query_body_facade_and_selectors() {
        let root = tempfile::tempdir().unwrap();
        for body in [false, true] {
            let api = fixture(body);
            let tree = render_sdk(
                &api,
                if body { "body" } else { "query" },
                Some("paging-sdk"),
                SdkClientStyle::Namespaced,
            )
            .unwrap();
            tree.write_to(
                root.path()
                    .join(if body { "body-output" } else { "query-output" }),
            )
            .unwrap();
        }
        let query = r#"require 'paging_sdk'
require 'json'
Response=Struct.new(:code,:body)
seen=[]
transport=lambda do |request|
 query=URI.decode_www_form(request.uri.query || '').to_h;page=query.fetch('page').to_i
 seen << page
 Response.new('200',JSON.generate({'items'=>page<3 ? [page] : []}))
end
client=PagingSdk::Client.new(base_url:'https://example.invalid',transport:transport)
raise unless client.list_items_pages.to_a.length==3 && seen==[1,2,3]
seen.clear;raise unless client.items.list_items_pages(page:0,limit:2).to_a.length==1 && seen==[0]
[true,-1,1.5].each do |bad|
 begin;client.list_items_pages(page:bad).to_a;raise 'bad page accepted';rescue ArgumentError;end
end
raise unless client.send(:kaji_json_path,[{'items'=>[1,2]}],'$[0].items[-1]')==2
raise unless client.send(:kaji_json_path,{'a/b'=>{'~items'=>[1]}},'/a~1b/~0items/0')==1
raise unless client.send(:kaji_json_path,[1,2],'$.items').nil?
raise unless client.send(:kaji_json_path,[1,2],'/-1').nil? && client.send(:kaji_json_path,[1,2],'/01').nil?
"#;
        let body = r#"require 'paging_sdk'
require 'json'
Response=Struct.new(:code,:body)
seen=[]
transport=lambda do |request|
 data=JSON.parse(request.body);query=URI.decode_www_form(request.uri.query || '').to_h
 seen << [data,query]
 Response.new('200',JSON.generate({'items'=>data.fetch('page')<3 ? [1,2] : [3]}))
end
client=PagingSdk::Client.new(base_url:'https://example.invalid',transport:transport)
original={'page'=>2,'limit'=>2,'filter'=>'keep'}
raise unless client.search_items_pages(body:original,page:77).to_a.length==2
raise unless seen.map{|entry|entry[0]['page']}==[2,3] && seen.all?{|entry|entry[0]['limit']==2 && entry[0]['filter']=='keep' && entry[1]['page']=='77'}
raise unless original=={'page'=>2,'limit'=>2,'filter'=>'keep'}
seen.clear;model=PagingSdk::Models::PageInput.from_hash({'limit'=>2,'filter'=>'keep'})
raise unless client.search_items_pages(body:model,page:88).to_a.length==3
raise unless !model.to_h.key?('page') && seen.map{|entry|entry[0]['page']}==[1,2,3]
"#;
        for (dir, script) in [("query", query), ("body", body)] {
            let output = std::process::Command::new("ruby")
                .args(["-Ilib", "-e", script])
                .current_dir(root.path().join(format!("{dir}-output")).join(dir))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    #[test]
    fn unsupported_declarations_are_diagnostic_and_not_guessed() {
        let mut api = fixture(false);
        api.operations[0]
            .annotations
            .get_mut("x-kaji-pagination")
            .unwrap()["type"] = serde_json::json!("cursor");
        let tree = render_sdk(&api, "ruby", Some("paging-sdk"), SdkClientStyle::Flat).unwrap();
        assert!(
            tree.get("ruby/.kaji/pagination-diagnostics.json")
                .unwrap()
                .contains("supports declared page")
        );
        assert!(
            !tree
                .get("ruby/lib/paging_sdk/client.rb")
                .unwrap()
                .contains("def list_items_pages")
        );
    }
}

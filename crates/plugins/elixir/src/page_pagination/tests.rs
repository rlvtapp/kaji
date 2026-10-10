use super::*;

#[test]
fn page_plan_emits_lazy_canonical_helpers_and_facades() {
    use poolster_core::{HttpMethod, OperationParameter, OperationResponse};
    let mut operation = Operation {
        id: "listPets".into(),
        method: HttpMethod::Get,
        path: "/pets".into(),
        ..Default::default()
    };
    operation.parameters = vec![OperationParameter {
        name: "page".into(),
        location: "query".into(),
        required: false,
        schema: Some(SchemaValue::new(SchemaKind::Integer)),
        description: None,
        annotations: Default::default(),
    }];
    operation.responses = vec![OperationResponse::json(
        "200",
        SchemaValue::new(SchemaKind::Array {
            items: Box::new(SchemaValue::new(SchemaKind::String)),
        }),
    )];
    operation.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page","in":"parameters"}],"outputs":{"results":"$"}}));
    let mut api = Api {
        operations: vec![operation],
        ..Default::default()
    };
    let source = render(&api, &api.operations[0]).unwrap().unwrap();
    assert!(source.contains("Stream.resource"));
    assert!(source.contains("Keyword.get(options, :page) || 1"));
    assert!(source.contains("count >= 10000"));
    assert!(source.contains("Client.json_path(response, [])"));
    assert!(render_api_facade("Pets", &api).contains("list_pets_pages"));
    assert!(
        render_resource_facade("Pets", &api, "Pets", &[&api.operations[0]])
            .contains("list_pets_pages")
    );
    api.operations[0].parameters[0].required = true;
    let source = render(&api, &api.operations[0]).unwrap().unwrap();
    assert!(!source.contains("|| 1"));
    api.operations[0].annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"}],"outputs":{"results":"$.missing"}}));
    assert!(render(&api, &api.operations[0]).is_err());
}

#[cfg(test)]
mod expanded_tests {
    use super::*;
    use poolster_core::{
        AdditionalProperties, Field, HttpMethod, OperationParameter, OperationResponse,
    };
    fn fixture(kind: &str) -> Api {
        let mut operation = Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets".into(),
            ..Default::default()
        };
        let (parameters, response, rule) = if kind == "offsetLimit" {
            (
                vec![
                    OperationParameter {
                        name: "offset".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::Integer)),
                        description: None,
                        annotations: Default::default(),
                    },
                    OperationParameter {
                        name: "limit".into(),
                        location: "query".into(),
                        required: false,
                        schema: Some(SchemaValue::new(SchemaKind::Integer)),
                        description: None,
                        annotations: Default::default(),
                    },
                ],
                SchemaValue::new(SchemaKind::Array {
                    items: Box::new(SchemaValue::new(SchemaKind::String)),
                }),
                serde_json::json!({"type":"offsetLimit","inputs":[{"name":"offset","type":"offset"},{"name":"limit","type":"limit"}],"outputs":{"results":"$"}}),
            )
        } else {
            (
                vec![],
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "next".into(),
                        value: SchemaValue::new(SchemaKind::String),
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
                serde_json::json!({"type":"url","outputs":{"nextUrl":"/next"}}),
            )
        };
        operation.parameters = parameters;
        operation.responses = vec![OperationResponse::json("200", response)];
        operation
            .annotations
            .insert("x-poolster-pagination".into(), rule);
        Api {
            operations: vec![operation],
            ..Default::default()
        }
    }
    #[test]
    fn offset_and_url_plans_emit_lazy_facades_and_origin_guard() {
        let api = fixture("offsetLimit");
        let helper = render(&api, &api.operations[0]).unwrap().unwrap();
        assert!(helper.contains("Keyword.get(options, :offset) || 0"));
        assert!(helper.contains("page + length(items)"));
        assert!(helper.contains(":invalid_pagination_offset"));
        let api = fixture("url");
        let helper = render(&api, &api.operations[0]).unwrap().unwrap();
        assert!(helper.contains("MapSet.member?"));
        assert!(helper.contains("_poolster_pagination_url"));
        assert!(render_api_facade("Probe", &api).contains("list_pets_pages"));
        assert!(
            render_operation("Probe", &api, &api.operations[0]).contains("{:poolster_url, url}")
        );
        let runtime = render_client("Probe");
        assert!(runtime.contains("next.port == base.port"));
        assert!(runtime.contains("next.userinfo == nil and next.fragment == nil"));
        assert!(runtime.contains(":unsafe_pagination_url"));
    }
    #[test]
    #[ignore = "Requires Elixir1.15+/OTP; dependency-free generated offset/URL security and laziness probe"]
    fn generated_offset_and_url_streams_preserve_laziness_and_auth_origin() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("multipart_body.ex"),
            include_str!("../../templates/multipart.ex.tmpl")
                .replace("__POOLSTER_MODULE__", "Probe"),
        )
        .unwrap();
        std::fs::write(root.path().join("client.ex"), render_client("Probe")).unwrap();
        let offset = fixture("offsetLimit");
        let url = fixture("url");
        std::fs::write(
            root.path().join("operations.ex"),
            render_operation_chunk("Probe", &url, &url.operations, 0),
        )
        .unwrap();
        let helper = render(&offset, &offset.operations[0]).unwrap().unwrap();
        let script = include_str!("../../tests/fixtures/expanded_pagination_probe.exs")
            .replace("__OFFSET_HELPER__", &helper);
        std::fs::write(root.path().join("probe.exs"), script).unwrap();
        let output = std::process::Command::new("elixir")
            .arg("probe.exs")
            .current_dir(root.path())
            .output()
            .expect("Elixir unavailable");
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    #[test]
    #[ignore = "requires Elixir toolchain"]
    fn generated_page_stream_executes_lazily() {
        use poolster_core::{HttpMethod, OperationParameter, OperationResponse};
        let mut operation = Operation {
            id: "listPets".into(),
            method: HttpMethod::Get,
            path: "/pets".into(),
            ..Default::default()
        };
        operation.parameters = vec![
            OperationParameter {
                name: "page".into(),
                location: "query".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            },
            OperationParameter {
                name: "limit".into(),
                location: "header".into(),
                required: false,
                schema: Some(SchemaValue::new(SchemaKind::Integer)),
                description: None,
                annotations: Default::default(),
            },
        ];
        operation.responses = vec![OperationResponse::json(
            "200",
            SchemaValue::new(SchemaKind::Array {
                items: Box::new(SchemaValue::new(SchemaKind::String)),
            }),
        )];
        operation.annotations.insert("x-poolster-pagination".into(), serde_json::json!({"type":"page","inputs":[{"name":"page","type":"page"},{"name":"limit","type":"limit"}],"outputs":{"results":"$"}}));
        let api = Api {
            operations: vec![operation],
            ..Default::default()
        };
        let helper = render(&api, &api.operations[0]).unwrap().unwrap();
        let script = format!(
            r#"
defmodule Client do
 def json_path(value, []), do: value
end
defmodule Probe do
 def list_pets(_client, options) do
  page = Keyword.fetch!(options, :page)
  Process.put(:pages, Process.get(:pages, []) ++ [page])
  if Keyword.get(options, :tenant) != "kept", do: raise("lost options")
  {{:ok, if(page == 0, do: ["a", "b"], else: ["c"])}}
 end
{helper}
end
stream = Probe.list_pets_pages(nil, page: 0, limit: 2, tenant: "kept")
if Process.get(:pages) != nil, do: raise("eager request")
if Enum.to_list(stream) != [{{:ok, ["a", "b"]}}, {{:ok, ["c"]}}], do: raise("pages")
if Process.get(:pages) != [0, 1], do: raise("controls")
Process.delete(:pages)
if Enum.to_list(Probe.list_pets_pages(nil, page: -1)) != [{{:error, :invalid_pagination_page}}], do: raise("negative page")
if Enum.to_list(Probe.list_pets_pages(nil, limit: 0)) != [{{:error, :invalid_pagination_limit}}], do: raise("invalid limit")
if Process.get(:pages) != nil, do: raise("invalid request executed")
"#
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("probe.exs");
        std::fs::write(&path, script).unwrap();
        let output = std::process::Command::new("elixir")
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

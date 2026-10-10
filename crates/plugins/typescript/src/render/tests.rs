use super::*;
use poolster_core::ast::{Field, HttpMethod, Operation, Schema};
use std::collections::BTreeMap;

fn value(kind: SchemaKind) -> SchemaValue {
    SchemaValue::new(kind)
}

fn api() -> Api {
    Api {
        name: "Pet Store".into(),
        version: "1.2.3".into(),
        annotations: BTreeMap::new(),
        schemas: vec![
            Schema::new(
                "Pet",
                value(SchemaKind::Object {
                    fields: vec![
                        Field {
                            name: "display-name".into(),
                            value: value(SchemaKind::String),
                            required: true,
                            annotations: BTreeMap::new(),
                        },
                        Field {
                            name: "class".into(),
                            value: value(SchemaKind::String),
                            required: false,
                            annotations: BTreeMap::new(),
                        },
                    ],
                    additional_properties: AdditionalProperties::Forbidden,
                }),
            ),
            Schema::new(
                "Pets",
                value(SchemaKind::Array {
                    items: Box::new(SchemaValue::reference("#/components/schemas/Pet")),
                }),
            ),
        ],
        operations: vec![Operation {
            id: "get-pets".into(),
            method: HttpMethod::Get,
            path: "/pets/{id}".into(),
            responses: vec![poolster_core::OperationResponse {
                status: "200".into(),
                description: None,
                media_types: vec![poolster_core::OperationMediaType {
                    content_type: "application/json".into(),
                    schema: Some(SchemaValue::reference("#/components/schemas/Pets")),
                }],
            }],

            annotations: BTreeMap::new(),
            ..Default::default()
        }],
    }
}

#[test]
fn renders_compositions_enums_and_safe_properties() {
    let mut status = value(SchemaKind::String);
    status.enum_values = vec![json!("active"), json!("deleted")];
    let mut nullable = value(SchemaKind::OneOf {
        variants: vec![SchemaValue::reference("Pet"), value(SchemaKind::Integer)],
    });
    nullable.nullable = true;
    let mut api = api();
    api.schemas.push(Schema::new("Status", status));
    api.schemas.push(Schema::new("Result", nullable));
    let output = render_models(&api);
    assert!(output.contains("export type Status = \"active\" | \"deleted\";"));
    assert!(output.contains("export type Result = Pet | number | null;"));
    assert!(output.contains("\"display-name\": string"));
    assert!(output.contains("\"class\"?: string"));
}

#[test]
fn emits_configurable_types_only_package() {
    let config = ArtifactOptions {
        package_name: Some("@acme/pets".into()),
        ..Default::default()
    };
    let package = TypeScriptPackage.generate(&api(), &config).unwrap();
    assert!(package[0].contents.contains("./models"));
    assert!(package[1].contents.contains("@acme/pets"));
    assert!(!package[1].contents.contains("peerDependencies"));
}

#[test]
fn generated_output_cannot_escape_configured_directory() {
    let config = ArtifactOptions {
        output_dir: Some("../escape".into()),
        ..Default::default()
    };
    assert!(TypeScriptModels.generate(&api(), &config).is_err());
}

#[test]
fn emits_the_remaining_poolster_style_artifact_slices() {
    let api = api();
    let mut zod_api = api.clone();
    zod_api.operations[0].request_body = Some(poolster_core::OperationRequestBody::json(
        SchemaValue::reference("#/components/schemas/Pet"),
        true,
    ));
    let config = ArtifactOptions::default();
    let zod = TypeScriptZod.generate(&zod_api, &config).unwrap();
    let react = TypeScriptReactQuery.generate(&api, &config).unwrap();
    let vue = TypeScriptVueQuery.generate(&api, &config).unwrap();
    let swr = TypeScriptSwr.generate(&api, &config).unwrap();
    let faker = TypeScriptFaker.generate(&api, &config).unwrap();
    let msw = TypeScriptMsw.generate(&api, &config).unwrap();
    let mut cypress_config = config.clone();
    cypress_config.cypress_options.include_mutations = true;
    cypress_config.cypress_options.operation_overrides.insert(
        "get-pets".into(),
        crate::CypressOperationOptions {
            path: Some("/pets/fixture".into()),
            ..Default::default()
        },
    );
    let cypress = TypeScriptCypress.generate(&api, &cypress_config).unwrap();
    let redoc = ReDoc.generate(&api, &config).unwrap();
    let mcp = McpToolManifest.generate(&api, &config).unwrap();

    assert_eq!(zod[0].path.to_string_lossy(), "typescript/zod.ts");
    assert!(zod[0].contents.contains("PetSchema = z.object"));
    assert!(
        zod[0]
            .contents
            .contains("export const GetPetsRequestBodySchemas")
    );
    assert!(
        zod[0]
            .contents
            .contains("export const GetPetsResponseSchemas")
    );
    assert!(zod[0].contents.contains("export const poolsterSchemas = {"));
    assert!(zod[0].contents.contains("\"Pet\": PetSchema"));
    assert!(
        zod[0]
            .contents
            .contains("export const poolsterOperationSchemas:")
    );
    assert!(zod[0].contents.contains("\"get-pets\": {"));
    assert!(zod[0].contents.contains("getPoolsterSchema"));
    assert!(zod[0].contents.contains("getPoolsterOperationSchemas"));
    assert!(zod[0].contents.contains("Standard Schema V1"));
    assert!(react[0].contents.contains("useQuery"));
    assert!(vue[0].contents.contains("@tanstack/vue-query"));
    assert!(swr[0].contents.contains("useSWR"));
    assert!(faker[0].contents.contains("createPet"));
    assert!(msw[0].contents.contains("/pets/:id"));
    assert!(cypress[0].contents.contains("cy.request"));
    assert!(cypress[0].contents.contains("reference types=\"cypress\""));
    assert!(
        cypress[0]
            .contents
            .contains("url: baseUrl + \"/pets/fixture\"")
    );
    assert!(!cypress[0].contents.contains("${$"));
    assert_eq!(redoc.len(), 2);
    assert!(redoc[0].contents.contains("<redoc"));
    assert!(mcp[0].contents.contains("getPets"));
    assert!(!mcp[0].contents.contains("\"body\""));
}

#[test]
fn zod_uses_zod_four_compatible_optional_and_literal_forms() {
    let mut nullish = value(SchemaKind::String);
    nullish.nullish = true;
    assert_eq!(render_zod(&nullish), "z.string().nullish()");

    let mut optional = value(SchemaKind::Integer);
    optional.optional = true;
    assert_eq!(render_zod(&optional), "z.number().int().optional()");

    // JSON Schema permits structured `const` values, while Zod's literal
    // schema accepts scalars only. Keep the object validator valid rather
    // than generating a Zod call which fails during module evaluation.
    let mut structured_constant = value(SchemaKind::Object {
        fields: Vec::new(),
        additional_properties: AdditionalProperties::Forbidden,
    });
    structured_constant.const_value = Some(json!({ "kind": "pet" }));
    assert_eq!(render_zod(&structured_constant), "z.object({  }).strict()");
}

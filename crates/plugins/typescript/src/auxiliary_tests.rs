//! Native behavior probes for isolated, bounded fixture generation.
#[test]
#[ignore = "requires Node, TypeScript and @faker-js/faker; set KAJI_TSC_JS and KAJI_TS_NODE_MODULES"]
fn fixture_constraints_seed_lossless_and_recursion_execute() {
    use std::{fs, process::Command};
    let directory = std::env::temp_dir().join(format!("kaji-aux-native-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("runtime.ts"),
        include_str!("fixture_runtime.ts.txt"),
    )
    .unwrap();
    fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    let modules = std::env::var("KAJI_TS_NODE_MODULES").expect("KAJI_TS_NODE_MODULES");
    #[cfg(unix)]
    {
        let _ = fs::remove_file(directory.join("node_modules"));
        std::os::unix::fs::symlink(modules, directory.join("node_modules")).unwrap();
    }
    let compiler = std::env::var("KAJI_TSC_JS").expect("KAJI_TSC_JS");
    let output = Command::new("node")
        .arg(compiler)
        .args([
            "--strict",
            "--skipLibCheck",
            "--target",
            "es2022",
            "--module",
            "nodenext",
            "--moduleResolution",
            "nodenext",
            "runtime.ts",
        ])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    fs::write(directory.join("probe.mjs"), r#"
import assert from 'node:assert/strict';
import {createFixtureRuntime} from './runtime.js';
const integer = (constraints={}, extensions={}) => ({kind:{kind:'integer'},constraints,extensions});
const string = (constraints={}) => ({kind:{kind:'string'},constraints});
const schemas = {
 Negative:integer({maximum:'-100000',multipleOf:'7'}),
 Wide:integer({minimum:'9007199254740993',maximum:'9007199254741003',multipleOf:'3'},{'x-kaji-integer':'bigint'}),
 WideString:integer({minimum:'9007199254740993',maximum:'9007199254741003'},{'x-kaji-integer':'string'}),
 Pattern:string({pattern:'^[A-Z]{4}$',minLength:4,maxLength:4}),
 Unique:{kind:{kind:'array',items:integer({minimum:'0',maximum:'10'})},constraints:{minItems:5,maxItems:5,uniqueItems:true}},
 Nullable:{kind:{kind:'any_of',variants:[string(),{kind:{kind:'null'}}]}},
 Recursive:{kind:{kind:'object',fields:[{name:'child',required:false,value:{kind:{kind:'reference',reference:'#/components/schemas/Recursive'}}}]}},
 Impossible:{kind:{kind:'object',fields:[{name:'child',required:true,value:{kind:{kind:'reference',reference:'#/components/schemas/Impossible'}}}]}},
 Overlap:{kind:{kind:'one_of',variants:[integer(),integer()]}},
};
const runtime=createFixtureRuntime(schemas,{seed:42,max_depth:6,max_attempts:64,overrides:{}});
const sequence=()=>Array.from({length:30},()=>[runtime.create('Negative'),String(runtime.create('Wide')),runtime.create('WideString'),runtime.create('Pattern')]);
const first=sequence();runtime.seed(42);assert.deepEqual(sequence(),first);
for(let i=0;i<100;i++){
 const negative=runtime.create('Negative');assert(negative<=-100000);assert(negative%7===0);
 const wide=runtime.create('Wide');assert(wide>=9007199254740993n&&wide<=9007199254741003n);assert.equal(wide%3n,0n);
 assert.match(runtime.create('Pattern'),/^[A-Z]{4}$/);
 const unique=runtime.create('Unique');assert.equal(unique.length,5);assert.equal(new Set(unique).size,5);
 runtime.create('Nullable');runtime.create('Recursive');
}
assert.throws(()=>runtime.create('Impossible'),/finite|required|fixture/);
assert.throws(()=>runtime.create('Overlap'),/fixture/);
console.log('bounded fixtures, constraint validation, exact integers and seed replay passed');
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        directory.display()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Node, TypeScript and MSW; set KAJI_TSC_JS and KAJI_TS_NODE_MODULES"]
fn msw_declared_response_scenarios_and_pagination_execute() {
    use kaji_core::{Api, HttpMethod, Operation, OperationResponse, SchemaValue};
    use std::{fs, process::Command};
    let mut operation = Operation {
        id: "listItems".into(),
        method: HttpMethod::Get,
        path: "http://kaji.test/items/{id}".into(),
        responses: vec![OperationResponse::json(
            "201",
            SchemaValue::new(kaji_core::SchemaKind::String),
        )],
        ..Default::default()
    };
    operation.annotations.insert("x-kaji-mock".into(),serde_json::json!({"scenarios":[
        {"name":"page-two","when":{"query":{"cursor":"next"},"path":{"id":"abc"},"headers":{"x-page":"yes"}},"response":{"status":200,"body":{"items":[2],"next":null},"delay_ms":1}},
        {"name":"limited","when":{"query":{"fail":"yes"}},"response":{"status":429,"headers":{"retry-after":"2"},"body":{"message":"slow down"}}},
        {"name":"empty","when":{"query":{"empty":"yes"}},"response":{"status":204}}
    ]}));
    let api = Api {
        operations: vec![operation],
        ..Default::default()
    };
    let generated = crate::render::TypeScriptMsw
        .generate(&api, &Default::default())
        .unwrap();
    let directory = std::env::temp_dir().join(format!("kaji-msw-native-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("msw.ts"), &generated[0].contents).unwrap();
    fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    #[cfg(unix)]
    {
        let _ = fs::remove_file(directory.join("node_modules"));
        std::os::unix::fs::symlink(
            std::env::var("KAJI_TS_NODE_MODULES").unwrap(),
            directory.join("node_modules"),
        )
        .unwrap();
    }
    let output = Command::new("node")
        .arg(std::env::var("KAJI_TSC_JS").unwrap())
        .args([
            "--strict",
            "--skipLibCheck",
            "--target",
            "es2022",
            "--module",
            "nodenext",
            "--moduleResolution",
            "nodenext",
            "msw.ts",
        ])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    fs::write(directory.join("probe.mjs"),r#"
import assert from 'node:assert/strict';
import {setupServer} from 'msw/node';
import {handlers} from './msw.js';
const server=setupServer(...handlers);server.listen({onUnhandledRequest:'error'});
try {
 const initial=await fetch('http://kaji.test/items/abc');assert.equal(initial.status,201);assert.match(initial.headers.get('content-type'),/json/);assert.equal(typeof await initial.json(),'string');
 const page=await fetch('http://kaji.test/items/abc?cursor=next',{headers:{'X-Page':'yes'}});assert.deepEqual(await page.json(),{items:[2],next:null});
 const mismatch=await fetch('http://kaji.test/items/other?cursor=next',{headers:{'x-page':'yes'}});assert.equal(mismatch.status,201);
 const limited=await fetch('http://kaji.test/items/abc?fail=yes');assert.equal(limited.status,429);assert.equal(limited.headers.get('retry-after'),'2');assert.equal((await limited.json()).message,'slow down');
 const named=await fetch('http://kaji.test/items/abc',{headers:{'x-kaji-mock-scenario':'limited'}});assert.equal(named.status,429);
 const empty=await fetch('http://kaji.test/items/abc?empty=yes');assert.equal(empty.status,204);assert.equal(await empty.text(),'');
 const unknown=await fetch('http://kaji.test/items/abc',{headers:{'x-kaji-mock-scenario':'missing'}});assert.equal(unknown.status,400);
} finally {server.close();}
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires installed Cypress browser; set KAJI_TS_NODE_MODULES and CYPRESS_CACHE_FOLDER"]
fn cypress_generated_smoke_executes_in_browser() {
    use kaji_core::{Api, HttpMethod, Operation, OperationResponse};
    use std::{collections::BTreeMap, fs, process::Command};
    let api = Api {
        name: "Native Smoke".into(),
        operations: vec![
            Operation {
                id: "readItem".into(),
                method: HttpMethod::Get,
                path: "/items/{id}".into(),
                responses: vec![OperationResponse {
                    status: "200".into(),
                    description: None,
                    media_types: vec![],
                }],
                ..Default::default()
            },
            Operation {
                id: "deleteItem".into(),
                method: HttpMethod::Delete,
                path: "/items/{id}".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let config = crate::render::ArtifactOptions {
        cypress_options: crate::CypressOptions {
            headers: BTreeMap::from([("x-test".into(), "authorized".into())]),
            operation_overrides: BTreeMap::from([(
                "readItem".into(),
                crate::CypressOperationOptions {
                    path: Some("/items/a%20b".into()),
                    query: BTreeMap::from([("cursor".into(), serde_json::json!("next"))]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        },
        ..Default::default()
    };
    let files = crate::render::TypeScriptCypress
        .generate(&api, &config)
        .unwrap();
    assert!(files[0].contents.contains("it.skip"));
    let directory =
        std::env::temp_dir().join(format!("kaji-cypress-native-{}", std::process::id()));
    fs::create_dir_all(directory.join("cypress/e2e")).unwrap();
    fs::write(directory.join("cypress/e2e/api.cy.ts"), &files[0].contents).unwrap();
    fs::write(directory.join("tsconfig.json"),r#"{"compilerOptions":{"target":"es2022","module":"esnext","moduleResolution":"bundler","strict":true,"skipLibCheck":true},"include":["cypress/**/*.ts"]}"#).unwrap();
    fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    #[cfg(unix)]
    {
        let _ = fs::remove_file(directory.join("node_modules"));
        std::os::unix::fs::symlink(
            std::env::var("KAJI_TS_NODE_MODULES").unwrap(),
            directory.join("node_modules"),
        )
        .unwrap();
    }
    fs::write(directory.join("probe.mjs"),r#"
import http from 'node:http';import {spawn} from 'node:child_process';import {writeFileSync} from 'node:fs';import assert from 'node:assert/strict';
let requests=0;
const server=http.createServer((request,response)=>{requests++;const url=new URL(request.url,'http://localhost');const valid=request.method==='GET'&&url.pathname==='/items/a%20b'&&url.searchParams.get('cursor')==='next'&&request.headers['x-test']==='authorized';response.writeHead(valid?200:500,{'content-type':'application/json'});response.end('{"ok":true}');});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const baseUrl=`http://127.0.0.1:${server.address().port}`;
writeFileSync('cypress.config.cjs',`module.exports={e2e:{supportFile:false,env:{API_BASE_URL:${JSON.stringify(baseUrl)}}},video:false}`);
try {
 const child=spawn(process.execPath,['node_modules/cypress/bin/cypress','run','--browser','electron','--headless','--spec','cypress/e2e/api.cy.ts'],{stdio:'inherit'});
 const timer=setTimeout(()=>child.kill('SIGKILL'),120000);
 const code=await new Promise(resolve=>child.once('exit',resolve));clearTimeout(timer);assert.equal(code,0);assert.equal(requests,1);
} finally {await new Promise(resolve=>server.close(resolve));}
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}\nGenerated fixture: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        directory.display()
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Node and Zod; set KAJI_TSC_JS and KAJI_TS_NODE_MODULES"]
fn zod_constraints_unions_and_lossless_boundaries_execute() {
    use kaji_core::{Api, Schema, SchemaKind, SchemaValue};
    use std::{fs, process::Command};
    let mut wide = SchemaValue::new(SchemaKind::Integer);
    wide.extensions
        .insert("x-kaji-integer".into(), serde_json::json!("bigint"));
    wide.constraints.extend([
        (
            "minimum".into(),
            serde_json::from_str("9007199254740993").unwrap(),
        ),
        (
            "maximum".into(),
            serde_json::from_str("9007199254741003").unwrap(),
        ),
        ("multipleOf".into(), serde_json::json!(3)),
    ]);
    let mut text = SchemaValue::new(SchemaKind::String);
    text.constraints.extend([
        ("minLength".into(), serde_json::json!(3)),
        ("maxLength".into(), serde_json::json!(4)),
        ("pattern".into(), serde_json::json!("^[A-Z]+$")),
    ]);
    let mut array = SchemaValue::new(SchemaKind::Array {
        items: Box::new(SchemaValue::new(SchemaKind::Integer)),
    });
    array.constraints.extend([
        ("minItems".into(), serde_json::json!(2)),
        ("uniqueItems".into(), serde_json::json!(true)),
    ]);
    let overlap = SchemaValue::new(SchemaKind::OneOf {
        variants: vec![
            SchemaValue::new(SchemaKind::Integer),
            SchemaValue::new(SchemaKind::Number),
        ],
    });
    let api = Api {
        schemas: vec![
            Schema::new("Wide", wide),
            Schema::new("Text", text),
            Schema::new("Unique", array),
            Schema::new("Exclusive", overlap),
            Schema::new(
                "Node",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![kaji_core::Field {
                        name: "child".into(),
                        value: SchemaValue::reference("#/components/schemas/Node"),
                        required: false,
                        annotations: Default::default(),
                    }],
                    additional_properties: Default::default(),
                }),
            ),
        ],
        ..Default::default()
    };
    let files = crate::render::TypeScriptZod
        .generate(
            &api,
            &crate::render::ArtifactOptions {
                layout: Some(crate::SourceLayout::SingleFile),
                ..Default::default()
            },
        )
        .unwrap();
    let directory = std::env::temp_dir().join(format!("kaji-zod-native-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("zod.ts"), &files[0].contents).unwrap();
    for file in crate::render::TypeScriptModels
        .generate(
            &api,
            &crate::render::ArtifactOptions {
                layout: Some(crate::SourceLayout::PerOperation),
                ..Default::default()
            },
        )
        .unwrap()
    {
        let target = directory.join(file.path.strip_prefix("typescript").unwrap());
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, file.contents).unwrap();
    }
    fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
    #[cfg(unix)]
    {
        let _ = fs::remove_file(directory.join("node_modules"));
        std::os::unix::fs::symlink(
            std::env::var("KAJI_TS_NODE_MODULES").unwrap(),
            directory.join("node_modules"),
        )
        .unwrap();
    }
    let output = Command::new("node")
        .arg(std::env::var("KAJI_TSC_JS").unwrap())
        .args([
            "--strict",
            "--skipLibCheck",
            "--target",
            "es2022",
            "--module",
            "esnext",
            "--moduleResolution",
            "bundler",
            "zod.ts",
            "models.ts",
        ])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    fs::write(directory.join("probe.mjs"),r#"
import assert from 'node:assert/strict';import {kajiSchemas} from './zod.js';
assert.equal(kajiSchemas.Wide.parse(9007199254740993n),9007199254740993n);
for(const value of [9007199254740992n,9007199254741004n,9007199254740994n,9007199254740993])assert.equal(kajiSchemas.Wide.safeParse(value).success,false);
assert.equal(kajiSchemas.Text.parse('ABC'),'ABC');for(const value of ['AB','ABCDE','abc'])assert.equal(kajiSchemas.Text.safeParse(value).success,false);
assert.deepEqual(kajiSchemas.Unique.parse([1,2]),[1,2]);for(const value of [[1],[1,1]])assert.equal(kajiSchemas.Unique.safeParse(value).success,false);
assert.equal(kajiSchemas.Exclusive.safeParse(1).success,false);assert.equal(kajiSchemas.Exclusive.safeParse(1.5).success,true);assert.deepEqual(kajiSchemas.Node.parse({child:{}}),{child:{}});
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn auxiliary_layout_modes_keep_atomic_models_and_stable_entrypoints() {
    use kaji_core::{Api, Schema, SchemaKind, SchemaValue, SourceLayout};
    let api = Api {
        schemas: vec![
            Schema::new(
                "First",
                SchemaValue::reference("#/components/schemas/Second"),
            ),
            Schema::new("Second", SchemaValue::new(SchemaKind::String)),
            Schema::new("Third", SchemaValue::new(SchemaKind::Integer)),
        ],
        ..Default::default()
    };
    for layout in [
        SourceLayout::SingleFile,
        SourceLayout::PerOperation,
        SourceLayout::chunked(1100),
        SourceLayout::per_resource(1100),
    ] {
        let options = crate::render::ArtifactOptions {
            layout: Some(layout.clone()),
            ..Default::default()
        };
        let files = crate::render::TypeScriptModels
            .generate(&api, &options)
            .unwrap();
        assert!(
            files
                .iter()
                .any(|file| file.path.to_string_lossy() == "typescript/models.ts")
        );
        if matches!(layout, SourceLayout::SingleFile) {
            assert_eq!(files.len(), 1);
        } else if matches!(layout, SourceLayout::PerOperation) {
            assert_eq!(files.len(), 4);
            assert!(files.iter().any(|file| {
                file.contents
                    .contains("import type { Second } from './chunk_0001'")
            }));
        }
        let files = crate::render::TypeScriptFaker
            .generate(&api, &options)
            .unwrap();
        assert!(
            files
                .iter()
                .any(|file| file.path.to_string_lossy() == "typescript/faker.ts")
        );
    }
}

#[test]
fn common_layout_inherits_and_plugin_override_wins() {
    use crate::composition::{faker, models, zod};
    use kaji_core::{
        Api, Schema, SchemaKind, SchemaValue,
        engine::{Common, Packages},
    };
    let api = Api {
        schemas: vec![
            Schema::new("First", SchemaValue::new(SchemaKind::String)),
            Schema::new("Second", SchemaValue::new(SchemaKind::Integer)),
        ],
        ..Default::default()
    };
    let directory = std::env::temp_dir().join(format!("kaji-layout-common-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    Packages::new()
        .common(Common::default().layout(crate::SourceLayout::PerOperation))
        .package(
            crate::package("aux")
                .with(models())
                .with(zod())
                .with(faker().layout(crate::SourceLayout::SingleFile)),
        )
        .package(crate::package("standalone").with(crate::types()))
        .package(
            crate::package("package-default")
                .common(Common::default().layout(crate::SourceLayout::SingleFile))
                .with(crate::types()),
        )
        .package(
            crate::package("override").with(crate::types().layout(crate::SourceLayout::SingleFile)),
        )
        .generate(&api, None)
        .unwrap()
        .write_to(&directory)
        .unwrap();
    assert!(directory.join("aux/zod_chunks/schemas_0001.ts").exists());
    assert!(!directory.join("aux/faker_chunks").exists());
    assert!(
        directory
            .join("standalone/models_chunks/chunk_0001.ts")
            .exists()
    );
    assert!(!directory.join("override/models_chunks").exists());
    assert!(!directory.join("package-default/models_chunks").exists());
    std::fs::remove_dir_all(directory).unwrap();
}

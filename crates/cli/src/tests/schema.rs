use super::recipes::combined_optional_packages;
use super::*;

#[test]
#[ignore = "requires Python jsonschema; set KAJI_TEST_PYTHON and PYTHONPATH"]
fn combined_optional_recipe_validates_actual_json_schema() {
    let config = serde_json::json!({"openapi":{"input":"api.yaml"},"output":{"path":"generated"},"packages":combined_optional_packages()});
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recipe.json");
    std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();
    let script = r#"
import json, sys, jsonschema
schema = json.load(open(sys.argv[1]))
config = json.load(open(sys.argv[2]))
validator = jsonschema.Draft202012Validator(schema)
validator.check_schema(schema)
assert validator.is_valid(config)
python = config['packages'][0]
terraform = config['packages'][1]['plugins'][0]
python['idempotency']['defaults']['enabled'] = 'yes'
assert not validator.is_valid(config)
python['idempotency']['defaults']['enabled'] = False
python['idempotency']['operations']['createItem']['header'] = 'bad header'
assert not validator.is_valid(config)
python['idempotency']['operations']['createItem']['header'] = 'X-Key'
python['api_reference'] = 'yes'
assert not validator.is_valid(config)
python['api_reference'] = True
terraform['data_sources'] = 'yes'
assert not validator.is_valid(config)
terraform['data_sources'] = True
java = {'language':'java', 'path':'java', 'plugins':[{'name':'sdk','preserve_presence':True,'open_enums':True}]}
config['packages'].append(java)
assert validator.is_valid(config)
java['plugins'][0]['preserve_presence'] = 'yes'
assert not validator.is_valid(config)
java['plugins'][0]['preserve_presence'] = True
resource = terraform['resources'][0]
resource['schema_version'] = 1
resource['state_upgrades'] = [{'version':0,'rename_fields':{'old_name':'name'}}]
assert validator.is_valid(config)
resource['state_upgrades'][0]['version'] = 'zero'
assert not validator.is_valid(config)
resource['state_upgrades'][0]['version'] = 0
resource.pop('id_parameter',None)
resource.pop('id_field',None)
resource['identity'] = [{'parameter':'organization','field':'organization_id'},{'parameter':'id','field':'id'}]
assert validator.is_valid(config)
resource['identity'] = resource['identity'][:1]
assert not validator.is_valid(config)
resource.pop('identity')
resource['polling'] = {'create': {'interval_ms':1,'max_attempts':3,'timeout_ms':1000,'success':[{'status':200},{'pointer':'/status','equals':'ready'}]},'delete':{'success':[{'status':404}]}}
assert validator.is_valid(config)
resource['polling']['create']['max_attempts'] = 0
assert not validator.is_valid(config)
resource['polling']['create']['max_attempts'] = 3
resource['polling']['create']['success'][1]['equals'] = []
assert not validator.is_valid(config)
resource['polling']['create']['success'][1]['equals'] = 'ready'
resource['polling']['create']['success'][1]['pointer'] = '/bad~2escape'
assert not validator.is_valid(config)
resource.pop('polling')
python['plugins'][1]['name'] = 'unsupported-operation-tests'
assert not validator.is_valid(config)
"#;
    let output = std::process::Command::new(
        std::env::var("KAJI_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
    )
    .args(["-c", script])
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/v1/poolster.schema.json"
    ))
    .arg(path)
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

use std::{fs, process::Command};

#[test]
#[ignore = "requires bundled Go compiler"]
fn local_child_edits_change_generation_provenance_and_sdk_types() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("api.yaml");
    let child = root.path().join("model.yaml");
    let output = root.path().join("sdk");
    fs::write(&source, "openapi: 3.2.0\ninfo: {title: Local, version: '1'}\ncomponents:\n  schemas:\n    Item: {$ref: './model.yaml#/Item'}\npaths:\n  /item:\n    get:\n      operationId: getItem\n      responses:\n        '200':\n          description: ok\n          content:\n            application/json:\n              schema: {$ref: '#/components/schemas/Item'}\n").unwrap();
    let generate = || {
        let result = Command::new(env!("CARGO_BIN_EXE_poolster"))
            .arg("generate")
            .arg(&source)
            .arg("--output")
            .arg(&output)
            .args(["--language", "python", "--name", "Local"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let lock: serde_json::Value = serde_json::from_slice(
            &fs::read(output.join(".poolster/generation.lock.json")).unwrap(),
        )
        .unwrap();
        lock["input"]["source_sha256"].as_str().unwrap().to_owned()
    };
    fs::write(
        &child,
        "Item:\n  type: object\n  required: [value]\n  properties:\n    value: {type: string}\n",
    )
    .unwrap();
    let first = generate();
    assert_eq!(generate(), first);
    fs::write(
        &child,
        "Item:\n  type: object\n  required: [value]\n  properties:\n    value: {type: integer}\n",
    )
    .unwrap();
    assert_ne!(generate(), first);
    let models = output.join("python/src/local_sdk/models");
    let rendered = fs::read_dir(models)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| fs::read_to_string(entry.path()).unwrap())
        .collect::<String>();
    assert!(rendered.contains("value: int"));
}

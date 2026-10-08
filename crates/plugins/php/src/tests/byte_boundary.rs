use super::*;

#[test]
#[ignore = "requires PHP8.2 and cached PSR dependencies"]
fn native_byte_bounded_operations_preserve_last_resource_and_transport() {
    let source = byte_boundary_api();
    let root = tempfile::tempdir().unwrap();
    let tree = render_sdk(
        &source,
        "php",
        Some("byte-probe"),
        SdkClientStyle::Namespaced,
    )
    .unwrap();
    let chunks = tree
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("ClientOperations"))
        .collect::<Vec<_>>();
    assert!(chunks.len() > 1);
    assert!(
        chunks
            .iter()
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    assert!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("ResourceOperations"))
            .all(|(_, contents)| contents.len() <= 128 * 1024)
    );
    tree.write_to(root.path()).unwrap();
    let namespace = namespace_for_package("byte-probe");
    let (resource, operations) = resource_operations(&source).into_iter().next().unwrap();
    let method = &operations.last().unwrap().1;
    let autoload = php_string(
        &std::env::var("POOLSTER_PHP_AUTOLOAD")
            .expect("POOLSTER_PHP_AUTOLOAD must point to cached PSR dependencies"),
    );
    let script = format!(
        r#"<?php
require {autoload};
spl_autoload_register(function($class) {{ $prefix='{namespace}\\'; if(str_starts_with($class,$prefix)) require __DIR__.'/src/'.str_replace('\\','/',substr($class,strlen($prefix))).'.php'; }});
$transport = new class implements \Psr\Http\Client\ClientInterface {{ public array $seen=[]; public function sendRequest(\Psr\Http\Message\RequestInterface $request): \Psr\Http\Message\ResponseInterface {{ $this->seen[]=(string)$request->getUri(); return new \Nyholm\Psr7\Response(200,['content-type'=>'application/json'],'"custom"'); }} }};
$client = new \{namespace}\Client(baseUrl:'https://unused.example', httpClient:$transport);
if ($client->getItem19() !== 'custom') throw new \Exception('last direct method');
$facade = new \{namespace}\Resources\{resource}Resource($client);
if ($facade->{method}() !== 'custom' || count($transport->seen)!==2 || !str_ends_with($transport->seen[1],'/items/19')) throw new \Exception('resource/transport forwarding');
"#
    );
    std::fs::write(root.path().join("php/probe.php"), script).unwrap();
    let output = std::process::Command::new("php")
        .arg(root.path().join("php/probe.php"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

use super::*;

#[test]
fn emits_operation_scoped_auto_idempotency_and_secure_uuid() {
    let source = idempotency_fixture();
    let tree = render_sdk(&source, "php", None, SdkClientStyle::Flat).unwrap();
    let op = tree.get("php/src/ClientOperations000.php").unwrap();
    assert!(op.contains("$xRequestKey ??= self::poolsterIdempotencyKey()"));
    assert!(op.contains("true, null, 'X-Request-Key'"));
    let client = tree.get("php/src/Client.php").unwrap();
    assert!(client.contains("random_bytes(16)"));
    assert!(client.contains("getHeaderLine('retry-after-ms')"));
    assert!(client.contains("[[$retryAfterMs, 1], [$retryAfter, 1000]]"));
    assert!(client.contains("strcasecmp($name, $idempotencyHeader)"));
    assert!(!client.contains("['GET', 'PUT', 'PATCH', 'DELETE']"));
}
#[test]
#[ignore = "requires PHP 8.2+; in-memory generated operation UUID probe"]
fn native_auto_idempotency_preserves_key_and_scopes_retry_header() {
    let source = idempotency_fixture();
    let tree = render_sdk(&source, "php", None, SdkClientStyle::Flat).unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let start = client
        .find("    private static function poolsterIdempotencyKey()")
        .unwrap();
    let end = client[start..]
        .find("    private function authHeaders()")
        .unwrap()
        + start;
    let operations = tree.get("php/src/ClientOperations000.php").unwrap();
    let trait_body =
        &operations[operations.find("{\n").unwrap() + 2..operations.rfind('}').unwrap()];
    let gate_start = client.find("    private function retryAllowed(").unwrap();
    let gate_end = client[gate_start..]
        .find("    private function retryableStatus(")
        .unwrap()
        + gate_start;
    let gate = &client[gate_start..gate_end];
    let script = format!(
        "<?php\nclass Probe {{ public array $seen=[];\n{}\n{trait_body}\nprivate function request(string $method,string $path,array $query,array $headers,mixed $body,string $kind,bool $retryable,?string $url,?string $idempotencyHeader):string {{ $this->seen[]=[$headers,$idempotencyHeader,$retryable];return '\\\"ok\\\"'; }} }}\n",
        &client[start..end]
    );
    let script=script.replace("public array $seen=[];",&format!("public array $seen=[];\n{gate}\npublic function canReplay(string $key,?string $header='X-Request-Key'):bool {{ return $this->retryAllowed('POST',['X-Request-Key'=>$key],$header); }}\n"));
    let script = script.replace("'\\\"ok\\\"'", "'\"ok\"'")
        + r#"$p=new Probe();$p->createItem();$p->createItem();$p->createItem('provided');$p->createItem('');if($p->canReplay('')||$p->canReplay(' ')||$p->canReplay('provided',null)||!$p->canReplay('provided')){throw new Exception('replay gate assertion');}$keys=array_map(fn($row)=>$row[0]['X-Request-Key'],$p->seen);if(!preg_match('/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/',$keys[0])||$keys[0]===$keys[1]||$keys[2]!=='provided'||$keys[3]!==''||$p->seen[0][1]!=='X-Request-Key'||!$p->seen[0][2]){throw new Exception('idempotency assertion');}"#;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe.php");
    std::fs::write(&path, script).unwrap();
    let output = std::process::Command::new("php")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires PHP 8.2+; deterministic native delay precedence probe"]
fn native_retry_after_ms_has_precedence_and_configured_cap() {
    let tree = render_sdk(&idempotency_fixture(), "php", None, SdkClientStyle::Flat).unwrap();
    let client = tree.get("php/src/Client.php").unwrap();
    let start = client.find("    private function retryDelay(").unwrap();
    let end = client[start..]
        .find("    /** @param array<string, mixed> $context */")
        .unwrap()
        + start;
    let script = format!(
        "<?php\nnamespace RetryProbe;function usleep(int $value):void {{ $GLOBALS['delays'][]=$value; }}\nclass Probe {{ private int $retryInitialDelayMs=250; private int $retryMaxDelayMs=400;\n{}\npublic function wait(string $seconds,string $ms):void {{ $this->retryDelay(0,$seconds,$ms); }} }}\n",
        &client[start..end]
    ) + r#"$GLOBALS['delays']=[];$p=new Probe();$p->wait('2','250');$p->wait('2','99999');$p->wait('.2','invalid');$p->wait('2','0');if($GLOBALS['delays']!==[250000,400000,200000]){throw new \Exception('retry delay assertion');}"#;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe.php");
    std::fs::write(&path, script).unwrap();
    let output = std::process::Command::new("php")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

//! Executes generated SDKs against a small Kaji contract mock.
//!
//! Docker is intentionally not required for this fast CI suite. The release
//! profile still emits the standalone httpmock package; this in-process HTTP
//! server exercises the exact same OpenAPI-derived route and response while
//! keeping language SDK verification quick and hermetic.

mod support;

use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    thread,
};

use kaji::{PackageOptions, ProfileSet, generate};

struct MockServer {
    base_url: String,
    worker: thread::JoinHandle<Vec<String>>,
}

impl MockServer {
    fn start(requests: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("the SDK contract mock should bind a loopback port");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let mut received = Vec::with_capacity(requests);
            for _ in 0..requests {
                let (mut stream, _) = listener.accept().expect("SDK should call the mock");
                let mut bytes = Vec::new();
                let mut buffer = [0_u8; 1024];
                while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut buffer).expect("read SDK request");
                    assert!(count > 0, "SDK closed the request before headers completed");
                    bytes.extend_from_slice(&buffer[..count]);
                }
                received.push(String::from_utf8(bytes).expect("SDK request must be UTF-8 headers"));
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{\"id\":\"contact_123\"}",
                    )
                    .expect("write mock response");
            }
            received
        });
        Self { base_url, worker }
    }

    fn finish(self) {
        let requests = self
            .worker
            .join()
            .expect("mock server worker should finish");
        assert_eq!(requests.len(), 2);
        for request in requests {
            assert!(
                request.starts_with("GET /v1/contacts/current HTTP/1.1"),
                "{request}"
            );
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer contract-test-token"),
                "SDK did not send its configured bearer credential: {request}"
            );
        }
    }
}

fn generated_contract_tree() -> kaji_core::GeneratedTree {
    generate(
        &support::sdk_contract_api(),
        ProfileSet::new("sdk")
            .go()
            .go_options(PackageOptions {
                package_name: Some("contractsdk".into()),
                ..PackageOptions::default()
            })
            .python()
            .python_options(PackageOptions {
                package_name: Some("contract-sdk".into()),
                ..PackageOptions::default()
            })
            .mock_server(),
    )
    .expect("the contract targets should generate")
}

fn require_command(program: &str) {
    let available = Command::new(program).arg("--version").output();
    assert!(
        available.is_ok(),
        "{program} is required for Kaji's generated-SDK contract suite"
    );
}

#[test]
#[ignore = "opens a loopback Kaji contract mock; CI runs this explicitly"]
fn generated_go_and_python_sdks_call_the_openapi_derived_mock() {
    require_command("go");
    require_command("python3");

    let output = tempfile::tempdir().expect("create SDK contract output directory");
    let tree = generated_contract_tree();
    assert!(
        tree.get("sdk/mock-server/fixtures/getcontact.yaml")
            .is_some()
    );
    tree.write_to(output.path())
        .expect("materialize generated SDK contract packages");

    let mock = MockServer::start(2);
    let go_test = r#"package contractsdk

import (
    "context"
    "os"
    "testing"
)

func TestKajiContractMock(t *testing.T) {
    client, err := NewClient(ClientConfig{
        BaseURL: os.Getenv("KAJI_CONTRACT_MOCK_URL"),
        APIKey: "contract-test-token",
        APIKeyHeader: "Authorization",
        APIKeyPrefix: "Bearer",
    })
    if err != nil { t.Fatal(err) }
    contact, err := client.GetContact(context.Background())
    if err != nil { t.Fatal(err) }
    if contact.ID != "contact_123" { t.Fatalf("contact = %#v", contact) }
}
"#;
    std::fs::write(output.path().join("sdk/go/contract_mock_test.go"), go_test)
        .expect("write Go contract test");
    let go_status = Command::new("go")
        .args(["test", "./..."])
        .current_dir(output.path().join("sdk/go"))
        .env("KAJI_CONTRACT_MOCK_URL", &mock.base_url)
        .env("GOCACHE", output.path().join("go-cache"))
        .status()
        .expect("run generated Go SDK test");
    assert!(go_status.success(), "generated Go SDK contract test failed");

    let python_program = r#"from contract_sdk import Client
import os

client = Client(os.environ["KAJI_CONTRACT_MOCK_URL"], api_key="contract-test-token")
contact = client.get_contact()
assert contact.id == "contact_123", contact
"#;
    let python_status = Command::new("python3")
        .arg("-c")
        .arg(python_program)
        .env("KAJI_CONTRACT_MOCK_URL", &mock.base_url)
        .env("PYTHONPATH", output.path().join("sdk/python/src"))
        .env("PYTHONPYCACHEPREFIX", output.path().join("python-cache"))
        .status()
        .expect("run generated Python SDK contract program");
    assert!(
        python_status.success(),
        "generated Python SDK contract program failed"
    );

    mock.finish();
}

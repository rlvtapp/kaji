use crate::TypeScript;
use anyhow::Result;
use kaji_core::{
    GeneratedFile,
    engine::{Meta, Plugin, PluginContext, Requirement},
};
pub struct Webhooks {
    models: Option<kaji_core::engine::Handle<crate::composition::Models>>,
    meta: Meta,
}
pub fn webhooks() -> Webhooks {
    Webhooks {
        meta: Meta::new(),
        models: None,
    }
}
impl Webhooks {
    pub fn using_models(
        mut self,
        models: kaji_core::engine::Handle<crate::composition::Models>,
    ) -> Self {
        self.models = Some(models);
        self
    }
}
impl Plugin<TypeScript> for Webhooks {
    fn kind(&self) -> &'static str {
        "typescript-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.models)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        cx.workspace
            .declare("webhooks", "verifyWebhook", "typescript-webhooks")?;
        cx.workspace.export("webhooks")?;
        cx.files.emit(GeneratedFile::new(
            "webhooks.ts",
            include_str!("webhooks.ts.txt"),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires KAJI_TSC_JS and Node"]
    fn native_standard_webhook_vectors_execute() {
        let api = kaji_core::Api {
            name: "Webhook".into(),
            schemas: vec![kaji_core::Schema::new(
                "Event",
                kaji_core::SchemaValue::new(kaji_core::SchemaKind::String),
            )],
            operations: vec![kaji_core::Operation {
                id: "getEvent".into(),
                method: kaji_core::HttpMethod::Get,
                path: "/event".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let tree = kaji_core::engine::Packages::new()
            .package(crate::package("sdk").with(crate::sdk()).with(webhooks()))
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        let cwd = dir.path().join("sdk");
        std::fs::write(
            cwd.join("vector.json"),
            include_str!("../../python/testdata/webhook-vectors.json"),
        )
        .unwrap();
        let compiler = std::env::var("KAJI_TSC_JS").unwrap();
        let output = std::process::Command::new("node")
            .args([&compiler, "-p", "tsconfig.json"])
            .current_dir(&cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(cwd.join("verify.mjs"),r#"import {verifyWebhook,WebhookVerificationError} from './dist/index.js';import fs from 'node:fs';import {webcrypto} from 'node:crypto';globalThis.crypto??=webcrypto;
const v=JSON.parse(fs.readFileSync('vector.json'));const body=new TextEncoder().encode(v.raw_body);const options={now:v.now};
if(JSON.stringify(await verifyWebhook(body,v.headers,[v.secret],options))!==JSON.stringify(v.payload))throw Error('vector');
await verifyWebhook(body,{...v.headers,'webhook-signature':'v2,ignored v1,bad '+v.headers['webhook-signature']},['whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4',v.secret],options);
for(const [raw,headers,keys,now] of [[new TextEncoder().encode(v.raw_body+' '),v.headers,[v.secret],v.now],[body,v.headers,[v.secret],v.now+301],[body,v.headers,[v.secret],v.now-301],[body,v.headers,[],v.now],[body,v.headers,['whsec_bad'],v.now],[body,{...v.headers,'WEBHOOK-ID':'duplicate'},[v.secret],v.now]]){try{await verifyWebhook(raw,headers,keys,{now});throw Error('accepted')}catch(e){if(!(e instanceof WebhookVerificationError)||e.message.includes(v.secret)||e.message.includes(v.raw_body))throw e;}}
const mutable=body.slice();const pending=verifyWebhook(mutable,v.headers,[v.secret],options);mutable.fill(0);if(JSON.stringify(await pending)!==JSON.stringify(v.payload))throw Error("mutable raw bytes changed authenticated payload");
await verifyWebhook(body,v.headers,[v.secret],{now:v.now,tolerance:0});
for(const options of [{now:NaN},{now:Infinity},{now:v.now,tolerance:NaN},{now:v.now,tolerance:-1}]){try{await verifyWebhook(body,v.headers,[v.secret],options);throw Error("accepted")}catch(e){if(!(e instanceof WebhookVerificationError))throw e;}}
"#).unwrap();
        let output = std::process::Command::new("node")
            .arg("verify.mjs")
            .current_dir(cwd)
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

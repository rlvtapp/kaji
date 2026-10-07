use crate::Rust;
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
impl Plugin<Rust> for Webhooks {
    fn kind(&self) -> &'static str {
        "rust-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.models)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        cx.workspace.webhooks = true;
        cx.files.emit(GeneratedFile::new(
            "src/webhooks.rs",
            include_str!("webhooks.rs.txt"),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires cached native Cargo dependencies"]
    fn native_standard_webhook_vectors_execute() {
        let api = kaji_core::Api {
            name: "Webhook".into(),
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
        let path = cwd.join("src/webhooks.rs");
        let mut source = std::fs::read_to_string(&path).unwrap();
        source.push_str(r#"
#[cfg(test)]mod vectors {use super::*;#[test]fn canonical_raw_rotation_and_failures(){let v:serde_json::Value=serde_json::from_str(include_str!("../vector.json")).unwrap();let raw=v["raw_body"].as_str().unwrap().as_bytes();let secret=v["secret"].as_str().unwrap();let headers=vec![("webhook-id",v["headers"]["webhook-id"].as_str().unwrap()),("webhook-timestamp",v["headers"]["webhook-timestamp"].as_str().unwrap()),("webhook-signature",v["headers"]["webhook-signature"].as_str().unwrap())];let opts=WebhookVerificationOptions{now:1700000000,tolerance:300};assert_eq!(verify_webhook(raw,&headers,&[secret],opts).unwrap(),v["payload"]);assert!(verify_webhook(raw,&headers,&["whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4",secret],opts).is_ok());let rotated=format!("v2,ignored v1,bad {}",headers[2].1);let mut rotations=headers.clone();rotations[2].1=&rotated;assert!(verify_webhook(raw,&rotations,&[secret],opts).is_ok());let unsupported=headers[2].1.replace("v1,","v1a,");let mut asymmetric=headers.clone();asymmetric[2].1=&unsupported;assert!(verify_webhook(raw,&asymmetric,&[secret],opts).is_err());let mut changed=raw.to_vec();changed.push(b' ');assert!(verify_webhook(&changed,&headers,&[secret],opts).is_err());for now in [1699999699,1700000301]{assert!(verify_webhook(raw,&headers,&[secret],WebhookVerificationOptions{now,..opts}).is_err());}for keys in [vec![],vec!["whsec_bad"]]{let error=verify_webhook(raw,&headers,&keys,opts).unwrap_err();assert_eq!(error.to_string(),"Webhook verification failed");}let mut duplicate=headers.clone();duplicate.push(("WEBHOOK-ID","duplicate"));assert!(verify_webhook(raw,&duplicate,&[secret],opts).is_err());assert!(verify_webhook(raw,&headers,&[secret],WebhookVerificationOptions{tolerance:0,..opts}).is_ok());}}
"#);
        std::fs::write(path, source).unwrap();
        let mut command = crate::native_cargo();
        command.args(["test", "--quiet"]);
        let output = command.current_dir(cwd).output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

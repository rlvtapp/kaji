use super::*;
use crate::package::RubyModels;
use poolster_core::engine::{Handle, Meta, Plugin, PluginContext, Requirement};
pub struct Webhooks {
    meta: Meta,
    models: Option<Handle<RubyModels>>,
}
pub fn webhooks() -> Webhooks {
    Webhooks {
        meta: Meta::new(),
        models: None,
    }
}
impl Webhooks {
    pub fn using_models(mut self, models: Handle<RubyModels>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models_from(self, sdk: &crate::Sdk) -> Self {
        self.using_models(sdk.models())
    }
}
impl Plugin<crate::Ruby> for Webhooks {
    fn kind(&self) -> &'static str {
        "ruby-webhooks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.models)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Ruby>) -> Result<()> {
        let models = cx.inputs.get::<RubyModels>()?;
        cx.files.emit(GeneratedFile::new(
            format!("lib/{}/webhooks.rb", models.import),
            include_str!("../templates/webhooks.rb").replace("__MODULE__", &models.module),
        )?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_webhook_verifier_executes_native_raw_rotation_and_failure_cases() {
        use poolster_core::engine::Packages;
        use poolster_core::{AdditionalProperties, Field, Schema, SchemaKind, SchemaValue};
        let api = Api {
            name: "Security".into(),
            schemas: vec![Schema::new(
                "WebhookEvent",
                SchemaValue::new(SchemaKind::Object {
                    fields: vec![Field {
                        name: "value".into(),
                        value: SchemaValue::new(SchemaKind::Integer),
                        required: true,
                        annotations: Default::default(),
                    }],
                    additional_properties: AdditionalProperties::Any,
                }),
            )],
            ..Default::default()
        };
        let sdk = crate::sdk();
        let verifier = webhooks().models_from(&sdk);
        let tree = Packages::new()
            .package(
                crate::package("sdk")
                    .name("security-sdk")
                    .with(verifier)
                    .with(sdk),
            )
            .generate(&api, None)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        std::fs::write(
            dir.path().join("sdk/vector.json"),
            include_str!("../../python/testdata/webhook-vectors.json"),
        )
        .unwrap();
        let script = r#"require 'security_sdk'
require 'security_sdk/webhooks'
vector=JSON.parse(File.read('vector.json'));body=vector.fetch('raw_body');headers=vector.fetch('headers');secret=vector.fetch('secret');now=vector.fetch('now')
raise unless SecuritySdk::Webhooks.verify(body,headers,[secret],now:now)==vector['payload']
model=SecuritySdk::Webhooks.verify_and_decode(body,headers,[secret],model:SecuritySdk::Models::WebhookEvent,now:now);raise unless model.value==1
rotated=headers.merge('webhook-signature'=>'v2,ignored v1,bad '+headers['webhook-signature'])
raise unless SecuritySdk::Webhooks.verify(body,rotated,['whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4',secret],now:now)==vector['payload']
cases=[[body+' ',headers,[secret],now],[body,headers,[secret],now+301],[body,headers,[secret],now-301],[body,headers,[],now],[body,headers,['whsec_bad'],now],[body,headers.merge('WEBHOOK-ID'=>'duplicate'),[secret],now],[body,headers.merge('webhook-signature'=>headers['webhook-signature'].sub('v1,','v1a,')),[secret],now]]
cases.each do |raw,metadata,keys,clock|
 begin;SecuritySdk::Webhooks.verify(raw,metadata,keys,now:clock);raise 'invalid webhook accepted';rescue ArgumentError=>error;raise 'secret leaked' if error.message.include?(secret)||error.message.include?(body);end
end
begin;SecuritySdk::Webhooks.verify(body,headers,[secret],now:now,tolerance:Float::NAN);raise 'invalid tolerance accepted';rescue ArgumentError;end
begin;SecuritySdk::Webhooks.verify(body,headers,[secret],now:Float::INFINITY);raise 'invalid clock accepted';rescue ArgumentError;end
"#;
        let output = std::process::Command::new("ruby")
            .args(["-Ilib", "-e", script])
            .current_dir(dir.path().join("sdk"))
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

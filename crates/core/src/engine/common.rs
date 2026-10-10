//! Shared package policy, overlaid without losing explicit local settings.
use crate::SdkClientStyle;

/// Optional shared settings. Unset values remain unset until a plugin applies
/// its defaults; false/flat/empty explicit values are never treated as absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Common {
    pub client_name: Option<String>,
    pub client_style: Option<SdkClientStyle>,
    pub package_version: Option<String>,
    pub layout: Option<crate::SourceLayout>,
    pub source_quality: Option<crate::SourceQuality>,
}

impl Common {
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = Some(name.into());
        self
    }
    pub fn client_style(mut self, style: SdkClientStyle) -> Self {
        self.client_style = Some(style);
        self
    }
    pub fn package_version(mut self, version: impl Into<String>) -> Self {
        self.package_version = Some(version.into());
        self
    }
    pub fn layout(mut self, layout: crate::SourceLayout) -> Self {
        self.layout = Some(layout);
        self
    }
    pub fn source_quality(mut self, quality: crate::SourceQuality) -> Self {
        self.source_quality = Some(quality);
        self
    }
    pub fn overlay(&self, local: &Self) -> Self {
        Self {
            client_name: local
                .client_name
                .clone()
                .or_else(|| self.client_name.clone()),
            client_style: local.client_style.or(self.client_style),
            layout: local.layout.clone().or_else(|| self.layout.clone()),
            source_quality: local
                .source_quality
                .clone()
                .or_else(|| self.source_quality.clone()),
            package_version: local
                .package_version
                .clone()
                .or_else(|| self.package_version.clone()),
        }
    }
}

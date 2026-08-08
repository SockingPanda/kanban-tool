use kanban_protocol::{
    WEB_ARTIFACT_BASE_PATH, WEB_PROTOCOL_VERSION, WebArtifactManifest, WebRuntimeConfig,
    validate_web_artifact_manifest,
};

/// Desktop 只连接这个固定的本机 host；不能因为冲突而随机改端口。
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8721;

/// 从 host 返回的三份事实中构造已经兼容的连接描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCompatibility {
    runtime: WebRuntimeConfig,
    manifest: WebArtifactManifest,
}

impl HostCompatibility {
    pub fn verify(
        runtime: WebRuntimeConfig,
        manifest: WebArtifactManifest,
        expected_server_version: &str,
    ) -> Result<Self, CompatibilityError> {
        if runtime.api_base_url != "" {
            return Err(CompatibilityError::new(
                "host runtime apiBaseUrl 必须为空并使用同源 API",
            ));
        }
        if runtime.web_base_path != WEB_ARTIFACT_BASE_PATH {
            return Err(CompatibilityError::new(format!(
                "host runtime webBasePath 必须为 {WEB_ARTIFACT_BASE_PATH:?}"
            )));
        }
        if runtime.server_version != expected_server_version {
            return Err(CompatibilityError::new(format!(
                "host serverVersion 不匹配: {} != {expected_server_version}",
                runtime.server_version
            )));
        }
        if runtime.protocol_version != WEB_PROTOCOL_VERSION {
            return Err(CompatibilityError::new(format!(
                "host protocolVersion 不匹配: {} != {WEB_PROTOCOL_VERSION}",
                runtime.protocol_version
            )));
        }
        if manifest.server_version != expected_server_version {
            return Err(CompatibilityError::new(format!(
                "Web artifact serverVersion 不匹配: {} != {expected_server_version}",
                manifest.server_version
            )));
        }
        validate_web_artifact_manifest(&manifest)
            .map_err(|error| CompatibilityError::new(error.to_string()))?;
        if runtime.web_build_id != manifest.build_id {
            return Err(CompatibilityError::new(format!(
                "host webBuildId 与 manifest buildId 不匹配: {} != {}",
                runtime.web_build_id, manifest.build_id
            )));
        }
        Ok(Self { runtime, manifest })
    }

    pub fn runtime(&self) -> &WebRuntimeConfig {
        &self.runtime
    }

    pub fn manifest(&self) -> &WebArtifactManifest {
        &self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityError(String);

impl CompatibilityError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CompatibilityError {}

#[cfg(test)]
mod tests {
    use kanban_protocol::{
        WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION, WebArtifactFile,
        web_artifact_build_id_for,
    };

    use super::*;

    fn compatible_values() -> (WebRuntimeConfig, WebArtifactManifest) {
        let payload = WebArtifactFile {
            path: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            bytes: 1,
            sha256: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
        };
        let build_id = web_artifact_build_id_for(
            WEB_ARTIFACT_FORMAT_VERSION,
            WEB_ARTIFACT_BASE_PATH,
            WEB_ARTIFACT_ENTRYPOINT,
            "3.0.0",
            WEB_PROTOCOL_VERSION,
            std::slice::from_ref(&payload),
        )
        .expect("build id");
        let manifest = WebArtifactManifest {
            format_version: WEB_ARTIFACT_FORMAT_VERSION,
            base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            server_version: "3.0.0".to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            build_id: build_id.clone(),
            files: vec![payload],
        };
        let runtime = WebRuntimeConfig {
            api_base_url: String::new(),
            web_base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            actor: "local".to_owned(),
            default_board: "default".to_owned(),
            server_version: "3.0.0".to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            web_build_id: build_id,
        };
        (runtime, manifest)
    }

    #[test]
    fn compatible_host_requires_matching_protocol_and_artifact_build() {
        let (runtime, manifest) = compatible_values();
        let compatibility = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect("same-version host should attach");
        assert_eq!(compatibility.runtime().default_board, "default");
    }

    #[test]
    fn stale_protocol_is_not_an_attachable_host() {
        let (mut runtime, manifest) = compatible_values();
        runtime.protocol_version = "v0".to_owned();
        let error = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect_err("stale protocol must not attach");
        assert!(error.to_string().contains("protocolVersion"));
    }

    #[test]
    fn mismatched_web_build_is_not_an_attachable_host() {
        let (mut runtime, manifest) = compatible_values();
        runtime.web_build_id = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            .to_owned();
        let error = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect_err("different web artifact must not attach");
        assert!(error.to_string().contains("webBuildId"));
    }
}

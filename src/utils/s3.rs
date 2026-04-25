use aws_sdk_s3::{
    config::{Credentials, SharedCredentialsProvider},
    presigning::PresigningConfig,
};
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::{error::Error, utils::serdefmt::duration_seconds};

fn default_expires_in() -> chrono::Duration {
    // 1800 is the magic number defined in the readest
    chrono::Duration::seconds(1800)
}

#[derive(Debug, Deserialize)]
pub struct S3Settings {
    pub endpoint: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub bucket: String,
    #[serde(with = "duration_seconds", default = "default_expires_in")]
    pub presign_upload_expires_in: chrono::Duration,
    #[serde(with = "duration_seconds", default = "default_expires_in")]
    pub presign_download_expires_in: chrono::Duration,
}

#[derive(Debug)]
pub struct S3Client {
    inner: aws_sdk_s3::Client,
    bucket: String,
    pub presign_upload_expires_in: std::time::Duration,
    pub presign_download_expires_in: std::time::Duration,
}

impl S3Client {
    pub async fn new(setting: S3Settings) -> Result<Self, Error> {
        // Build SDK config loader
        let credential_provider = SharedCredentialsProvider::new(Credentials::new(
            setting.access_key_id,
            setting.secret_access_key,
            None,
            None,
            "static-credentials",
        ));
        let config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(credential_provider)
            .region(aws_config::Region::new(setting.region))
            .endpoint_url(setting.endpoint);
        let sdk_config = config_loader.load().await;
        let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
            .force_path_style(true)
            .build();

        Ok(Self {
            inner: aws_sdk_s3::Client::from_conf(s3_config),
            bucket: setting.bucket,
            presign_upload_expires_in: setting.presign_upload_expires_in.to_std().unwrap(),
            presign_download_expires_in: setting.presign_download_expires_in.to_std().unwrap(),
        })
    }

    #[instrument(skip(self))]
    pub async fn presign_upload_url(
        &self,
        file_key: &str,
        content_length: i64,
    ) -> Result<String, Error> {
        self.inner
            .put_object()
            .bucket(&self.bucket)
            .key(file_key)
            .content_length(content_length)
            .presigned(PresigningConfig::expires_in(
                self.presign_upload_expires_in,
            )?)
            .await
            .map(|req| {
                let presigned_uri = req.uri().to_string();
                debug!(presigned_uri, "generate presigned upload url");
                presigned_uri
            })
            .map_err(|e| Error::S3Sdk(Box::new(aws_sdk_s3::Error::from(e))))
    }

    #[instrument(skip(self))]
    pub async fn presign_download_url(&self, file_key: &str) -> Result<String, Error> {
        self.inner
            .get_object()
            .bucket(&self.bucket)
            .key(file_key)
            .presigned(PresigningConfig::expires_in(
                self.presign_download_expires_in,
            )?)
            .await
            .map(|req| {
                let presigned_uri = req.uri().to_string();
                debug!(presigned_uri, "generate presigned download url");
                presigned_uri
            })
            .map_err(|e| Error::S3Sdk(Box::new(aws_sdk_s3::Error::from(e))))
    }

    #[instrument(skip(self))]
    pub async fn delete_object(&self, file_key: &str) -> Result<(), Error> {
        let output = self
            .inner
            .delete_object()
            .bucket(&self.bucket)
            .key(file_key)
            .send()
            .await;

        debug!(?output, "delete object");

        Ok(())
    }
}

use crate::config_loader::IpCameraSpecificConfig;
use crate::core::capture_source::{CaptureSource, FrameData, FrameDataBundle};
use anyhow::{Result, anyhow, Context};
use async_trait::async_trait;
use log::{debug, error, info};
use reqwest::Client;
use std::env;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use diqwest::WithDigestAuth;

pub struct IpCameraDevice {
    pub name: String,
    pub config: IpCameraSpecificConfig,
    // Maybe an Arc<Client> if we want to share it across multiple captures for the same device.
    // For now, each capture_image call will create a new client or use a shared one passed in.
    // Let's assume client is created per operation for simplicity now.
}

impl IpCameraDevice {
    pub fn new(name: String, config: IpCameraSpecificConfig) -> Self {
        Self {
            name,
            config,
        }
    }

    pub fn get_password(&self) -> Result<String> {
        let env_var_name = format!("{}_PASSWORD", self.name.to_uppercase().replace("-", "_"));
        env::var(&env_var_name)
            .with_context(|| format!("Password for camera '{}' not found in environment variable '{}'", self.name, env_var_name))
    }

    pub fn get_rtsp_url(&self) -> Result<String> {
        let username = self.config.username.as_ref()
            .ok_or_else(|| anyhow!("Username not configured for RTSP for camera '{}'", self.name))?;
        let password = self.get_password()
            .with_context(|| format!("Failed to get password for RTSP URL construction for camera '{}'", self.name))?;
        let ip = &self.config.ip;
        let port = self.config.rtsp_port.unwrap_or(554); // Default RTSP port
        let path = self.config.rtsp_path.as_deref()
            .ok_or_else(|| anyhow!("RTSP path not configured for camera '{}'", self.name))?;
        
        // Ensure path starts with a slash if not empty
        let formatted_path = if !path.is_empty() && !path.starts_with('/') {
            format!("/{}", path)
        } else {
            path.to_string()
        };

        Ok(format!("rtsp://{}:{}@{}:{}{}", username, password, ip, port, formatted_path))
    }
}

#[async_trait]
impl CaptureSource for IpCameraDevice {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_type(&self) -> String {
        "ip-camera".to_string()
    }

    async fn capture_image(
        &mut self, 
        output_dir: &Path, 
        timestamp_str: &str,
        image_format_config: &str, // e.g. "png" or "jpg"
        _jpeg_quality: Option<u8>, // TODO: Use these if image crate is used for re-saving/conversion
        _png_compression: Option<u32>,
    ) -> Result<FrameDataBundle> {
        debug!("IP Cam [{}]: Capturing image via HTTP CGI.", self.name);
        let client = Client::new(); // Consider sharing client if making many requests
        
        let username = self.config.username.as_ref()
            .ok_or_else(|| anyhow!("Username not configured for camera '{}'", self.name))?;
        let password = self.get_password()
            .with_context(|| format!("Failed to get password for camera '{}'", self.name))?;
        
        let url = format!("http://{}/cgi-bin/snapshot.cgi?channel=1", self.config.ip);
        info!("IP Cam [{}]: Requesting snapshot from {}", self.name, url);

        let resp_result = client.get(&url)
            .send_with_digest_auth(username, &password)
            .await;

        let image_content_bytes = match resp_result {
            Ok(response) => {
                if !response.status().is_success() {
                    error!("IP Cam [{}]: HTTP request failed with status: {}", self.name, response.status());
                    return Err(anyhow!("HTTP request failed for {} with status: {}", self.name, response.status()));
                }
                debug!("IP Cam [{}]: HTTP request successful (Status: {}). Reading bytes...", self.name, response.status());
                match response.bytes().await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("IP Cam [{}]: Failed to get bytes from HTTP response: {}", self.name, e);
                        return Err(anyhow!("Failed to get bytes from {}: {}", self.name, e));
                    }
                }
            },
            Err(e) => {
                error!("IP Cam [{}]: HTTP request send failed: {}", self.name, e);
                return Err(anyhow!("HTTP send failed for {}: {}", self.name, e));
            }
        };

        debug!("IP Cam [{}]: Received {} bytes from HTTP.", self.name, image_content_bytes.len());

        let filename = format!("{}_{}.{}", self.name, timestamp_str, image_format_config);
        let file_path = output_dir.join(&filename);

        match File::create(&file_path).await {
            Ok(mut f) => {
                if let Err(e) = f.write_all(&image_content_bytes).await {
                    error!("IP Cam [{}]: Failed to write image to {}: {}", self.name, file_path.display(), e);
                    return Err(anyhow!("Failed to write image for {}: {}", self.name, e));
                }
            }
            Err(e) => {
                error!("IP Cam [{}]: Failed to create file {}: {}", self.name, file_path.display(), e);
                return Err(anyhow!("Failed to create file for {}: {}", self.name, e));
            }
        }
        info!("✅ IP Cam [{}]: Saved snapshot ({} bytes) to {}", self.name, image_content_bytes.len(), file_path.display());

        Ok(FrameDataBundle {
            frames: vec![FrameData::IpCameraImage {
                name: self.name.clone(),
                path: file_path,
                format: image_format_config.to_string(),
            }],
        })
    }
}
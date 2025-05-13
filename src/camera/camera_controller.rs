use crate::camera::camera_entity::CameraEntity;
use anyhow::{Result, Context, anyhow, bail};
use log::{info, error, debug, warn};
use reqwest::{Client, StatusCode};
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::app_config::ApplicationConfig;
use std::time::Instant;

#[derive(Clone)]
pub struct CameraController {
    http_client: Client,
}

impl CameraController {
    pub fn new() -> Self {
        debug!("🔧 Initializing CameraController...");
        let start_time = Instant::now();
        let controller = CameraController {
            http_client: Client::new(),
        };
        debug!("✅ CameraController initialized in {:?}", start_time.elapsed());
        controller
    }

    pub async fn get_camera_time(&self, camera: &CameraEntity, app_config: &ApplicationConfig) -> Result<DateTime<Utc>> {
        let cam_name = &camera.config.name;
        debug!("⏱️ Attempting to get time for camera (HTTP CGI): {}", cam_name);
        let overall_start_time = Instant::now();

        let cgi_path = &app_config.cgi_time_path;
        let url = format!("http://{}{}", camera.config.ip, cgi_path);
        let username = &camera.config.username;
        let password = camera.get_password()
            .ok_or_else(|| anyhow!("🔑❌ Password not available for HTTP CGI request for camera '{}'", cam_name))?;

        debug!("  Making GET request to {} for camera time ({})", url, cam_name);
        let req_start_time = Instant::now();

        let response_res = self.http_client
            .get(&url)
            .basic_auth(username, Some(password))
            .send()
            .await
            .with_context(|| format!("HTTP GET request to {} failed for '{}' 📡💥", url, cam_name));

        let mut response = match response_res {
            Ok(resp) => resp,
            Err(e) => {
                error!("  ❌ Initial HTTP request for time failed for '{}' in {:?}: {:#}", cam_name, req_start_time.elapsed(), e);
                return Err(e);
            }
        };
        debug!("  Initial HTTP request for '{}' completed in {:?}, status: {}", cam_name, req_start_time.elapsed(), response.status());

        if response.status() == StatusCode::UNAUTHORIZED {
            info!("🛡️ Basic auth failed (401) for {}, attempting digest auth for camera: {}", url, cam_name);
            let digest_req_start_time = Instant::now();
            response = self.http_client
                .get(&url)
                .basic_auth(username, Some(password))
                .send()
                .await
                .with_context(|| format!("Digest auth HTTP GET request to {} failed for '{}' 🛡️💥", url, cam_name))?;
            debug!("  Digest auth HTTP request for '{}' completed in {:?}, status: {}", cam_name, digest_req_start_time.elapsed(), response.status());
        }

        if !response.status().is_success() {
            let status = response.status();
            let body_text_res = response.text().await;
            let body_text = body_text_res.as_deref().unwrap_or("<failed to read body>");
            error!(
                "❌ HTTP CGI get_time failed for '{}'. Status: {}. URL: {}. Body: {}. Total time: {:?}",
                cam_name, status, url, body_text, overall_start_time.elapsed()
            );
            bail!(
                "HTTP CGI get_time failed for '{}'. Status: {}. URL: {}. Body: {}",
                cam_name, status, url, body_text
            );
        }

        let body_read_start_time = Instant::now();
        let body = response.text().await
            .with_context(|| format!("Failed to read response body from {} for '{}' after successful status 📄💥", url, cam_name))?;
        debug!("  Read response body for '{}' in {:?}. Length: {} bytes", cam_name, body_read_start_time.elapsed(), body.len());
        
        let cleaned_body = body.trim().replace("'", "").replace("\"", "");
        debug!("  Cleaned body for parsing: '{}'", cleaned_body);
        
        // Try parsing common timestamp formats
        let formats_to_try = [
            "%Y-%m-%d %H:%M:%S",       // Common space separated
            "%Y-%m-%dT%H:%M:%SZ",      // ISO8601 Z_ulo_offset
            "%Y-%m-%dT%H:%M:%S%z",     // ISO8601 with offset
            "%Y-%m-%d %H:%M:%S%z",      // Space separated with offset
        ];

        // Attempt to parse the whole cleaned_body first
        for fmt in formats_to_try {
            if let Ok(naive_dt) = NaiveDateTime::parse_from_str(&cleaned_body, fmt) {
                let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                info!("✅ Successfully parsed time for camera '{}' (format: '{}'): {}. Total time: {:?}", cam_name, fmt, datetime_utc, overall_start_time.elapsed());
                return Ok(datetime_utc);
            }
        }

        // Attempt to find and parse a timestamp-like substring
        // This is less reliable and more of a fallback.
        if let Some(ts_str) = cleaned_body.split_whitespace().find(|s| formats_to_try.iter().any(|fmt| NaiveDateTime::parse_from_str(s, fmt).is_ok())) {
            for fmt in formats_to_try {
                if let Ok(naive_dt) = NaiveDateTime::parse_from_str(ts_str, fmt) {
                    let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                    info!("✅ Successfully parsed time substring '{}' for camera '{}' (format: '{}'): {}. Total time: {:?}", ts_str, cam_name, fmt, datetime_utc, overall_start_time.elapsed());
                    return Ok(datetime_utc);
                }
            }
        }
        
        // Try to extract from var assignments like `var CurrentTime = '...';`
        if let Some(start_idx) = cleaned_body.find("=") {
            let potential_time_part = cleaned_body[start_idx+1..].trim().trim_matches(|c: char| c == '\'' || c == '"' || c == ';');
            for fmt in formats_to_try {
                 if let Ok(naive_dt) = NaiveDateTime::parse_from_str(potential_time_part, fmt) {
                    let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                    info!("✅ Successfully parsed time from assignment for camera '{}' (format: '{}'): {}. Total time: {:?}", cam_name, fmt, datetime_utc, overall_start_time.elapsed());
                    return Ok(datetime_utc);
                }
            }
        }

        warn!(
            "⚠️ Could not find or parse a recognizable time string for '{}' from URL '{}'. Cleaned Body: '{}'. Total time: {:?}",
            cam_name, url, cleaned_body, overall_start_time.elapsed()
        );
        bail!(
            "Could not find or parse a recognizable time string in response from '{}' ({}). Cleaned Body: {}",
            cam_name, url, cleaned_body
        );
    }
}
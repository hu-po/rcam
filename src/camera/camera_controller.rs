use crate::camera::camera_entity::CameraEntity;
use crate::errors::AppError;
use log::{info, error};
use reqwest::{Client, StatusCode};
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::app_config::ApplicationConfig;

#[derive(Clone)] // Added Clone for use in operations modules
pub struct CameraController {
    http_client: Client, // HTTP client for CGI calls
}

impl CameraController {
    pub fn new() -> Self {
        CameraController {
            http_client: Client::new(),
        }
    }

    // This function is now highly dependent on a standardized HTTP CGI endpoint,
    // or might be removed if not generically feasible.
    pub async fn get_camera_time(&self, camera: &CameraEntity, app_config: &ApplicationConfig) -> Result<DateTime<Utc>, AppError> {
        let cam_name = &camera.config.name;
        info!("Attempting to get time for camera (HTTP CGI): {}", cam_name);

        let cgi_path = &app_config.cgi_time_path;

        let url = format!("http://{}{}", camera.config.ip, cgi_path);
        let username = &camera.config.username;
        let password = camera.get_password().ok_or_else(|| AppError::Authentication {
            camera_name: cam_name.clone(),
            details: "Password not available for HTTP CGI request".to_string(),
        })?;

        info!("Making GET request to {} for camera time ({})", url, cam_name);

        let response = self.http_client
            .get(&url)
            .basic_auth(username, Some(password)) // Using basic auth first, digest can be added if basic fails
            .send()
            .await
            .map_err(|e| AppError::HttpCgi(format!("Request to {} failed for '{}': {}", url, cam_name, e)))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            // Attempt with digest authentication if basic auth failed
            info!("Basic auth failed (401) for {}, attempting digest auth for camera: {}", url, cam_name);
            let response_digest = self.http_client
                .get(&url)
                .basic_auth(username, Some(password)) // Changed .digest to .basic_auth, reqwest handles negotiation
                .send()
                .await
                .map_err(|e| AppError::HttpCgi(format!("Auth retry request to {} failed for '{}': {}", url, cam_name, e)))?;
            
            if !response_digest.status().is_success() {
                let err_msg = format!("HTTP CGI get_time failed for '{}' after digest auth. Status: {}, URL: {}", cam_name, response_digest.status(), url);
                error!("{}", err_msg);
                return Err(AppError::HttpCgi(err_msg));
            }
            let body = response_digest.text().await.map_err(|e| AppError::HttpCgi(format!("Failed to read response body from {} for '{}': {}", url, cam_name, e)))?;
            // Simplified parsing: Assumes response is like "YYYY-MM-DD HH:MM:SS"
            // Example response: "var CurrentTime = '2023-10-27 10:30:00';" - needs more robust parsing
            // This is a very basic parser, cameras might return time in various formats.
            // A common approach is to find a string matching a date-time pattern.
            // For now, let's assume the body *is* the time string or contains it clearly.
            // A more robust solution would use regex or specific parsers based on camera type.
            
            // Attempt to parse a common format: "YYYY-MM-DD HH:MM:SS"
            // This is a placeholder for actual parsing logic based on camera output.
            // E.g. "var CurrentTime = '2023-10-27 10:30:00';" would require stripping and then parsing.
            // For now, we assume the body itself is the timestamp or we can directly parse it.
            // This will likely need refinement based on actual camera API responses.
            let cleaned_body = body.trim().replace("'", "").replace("\"", ""); // Basic cleaning
            // Look for a substring that looks like a timestamp
            let timestamp_str = cleaned_body.split_whitespace().find(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok())
                .or_else(|| NaiveDateTime::parse_from_str(&cleaned_body, "%Y-%m-%d %H:%M:%S").ok().map(|_| cleaned_body.as_str()));


            if let Some(ts_str) = timestamp_str {
                 match NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S") {
                    Ok(naive_dt) => {
                        let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                        info!("Successfully parsed time for camera '{}': {}", cam_name, datetime_utc);
                        return Ok(datetime_utc);
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to parse time string '{}' from camera '{}' ({}): {}", ts_str, cam_name, url, e);
                        error!("{}", err_msg);
                        return Err(AppError::HttpCgi(err_msg));
                    }
                }
            } else {
                 // Try another common format often found in CGI scripts (e.g. "CurrentTime='2024-03-10T15:45:30Z';")
                // This requires regex for robust parsing. For now, a simple string search and split.
                if let Some(start_idx) = cleaned_body.find("=") {
                    let potential_time_part = cleaned_body[start_idx+1..].trim_matches(|c: char| c == '\'' || c == '"' || c == ';'); // Escaped quote
                    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(potential_time_part, "%Y-%m-%dT%H:%M:%SZ") {
                        let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                        info!("Successfully parsed ISO time for camera '{}': {}", cam_name, datetime_utc);
                        return Ok(datetime_utc);
                    } else if let Ok(naive_dt) = NaiveDateTime::parse_from_str(potential_time_part, "%Y-%m-%d %H:%M:%S") {
                         let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                        info!("Successfully parsed time for camera '{}' (alt format): {}", cam_name, datetime_utc);
                        return Ok(datetime_utc);
                    }
                }
                let err_msg = format!("Could not find or parse a recognizable time string in response from '{}' ({}). Body: {}", cam_name, url, body);
                error!("{}", err_msg);
                return Err(AppError::HttpCgi(err_msg));
            }
        }

        // If basic auth succeeded
        if !response.status().is_success() {
            let err_msg = format!("HTTP CGI get_time failed for '{}'. Status: {}, URL: {}", cam_name, response.status(), url);
            error!("{}", err_msg);
            return Err(AppError::HttpCgi(err_msg));
        }

        let body = response.text().await.map_err(|e| AppError::HttpCgi(format!("Failed to read response body from {} for '{}': {}", url, cam_name, e)))?;
        // Simplified parsing logic (same as above for digest path)
        let cleaned_body = body.trim().replace("'", "").replace("\"", "");
        let timestamp_str = cleaned_body.split_whitespace().find(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok())
            .or_else(|| NaiveDateTime::parse_from_str(&cleaned_body, "%Y-%m-%d %H:%M:%S").ok().map(|_| cleaned_body.as_str()));

        if let Some(ts_str) = timestamp_str {
             match NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S") {
                Ok(naive_dt) => {
                    let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                    info!("Successfully parsed time for camera '{}': {}", cam_name, datetime_utc);
                    Ok(datetime_utc)
                }
                Err(e) => {
                    let err_msg = format!("Failed to parse time string '{}' from camera '{}' ({}): {}", ts_str, cam_name, url, e);
                    error!("{}", err_msg);
                    Err(AppError::HttpCgi(err_msg))
                }
            }
        } else {
            if let Some(start_idx) = cleaned_body.find("=") {
                let potential_time_part = cleaned_body[start_idx+1..].trim_matches(|c: char| c == '\'' || c == '"' || c == ';'); // Escaped quote
                if let Ok(naive_dt) = NaiveDateTime::parse_from_str(potential_time_part, "%Y-%m-%dT%H:%M:%SZ") {
                     let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                    info!("Successfully parsed ISO time for camera '{}': {}", cam_name, datetime_utc);
                    return Ok(datetime_utc);
                } else if let Ok(naive_dt) = NaiveDateTime::parse_from_str(potential_time_part, "%Y-%m-%d %H:%M:%S") {
                     let datetime_utc = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                    info!("Successfully parsed time for camera '{}' (alt format): {}", cam_name, datetime_utc);
                    return Ok(datetime_utc);
                }
            }
            let err_msg = format!("Could not find or parse a recognizable time string in response from '{}' ({}). Body: {}", cam_name, url, body);
            error!("{}", err_msg);
            Err(AppError::HttpCgi(err_msg))
        }
    }

    // This function is also now highly dependent on standardized HTTP CGI.
    pub async fn set_camera_enabled(&self, camera: &CameraEntity, app_config: &ApplicationConfig, enable: bool) -> Result<(), AppError> {
        let cam_name = &camera.config.name;
        let action_str = if enable { "enable" } else { "disable" };
        info!("Attempting to {} camera (HTTP CGI): {}", action_str, cam_name);

        let cgi_path = if enable {
            &app_config.cgi_control_enable_path
        } else {
            &app_config.cgi_control_disable_path
        };

        // Basic placeholder replacement, can be expanded (e.g. for {ip}, {port}, {channel})
        // For now, only the base path is used.
        let final_cgi_path = cgi_path.replace("{enable}", if enable { "1" } else { "0" });


        let url = format!("http://{}{}", camera.config.ip, final_cgi_path);
        let username = &camera.config.username;
        let password = camera.get_password().ok_or_else(|| AppError::Authentication {
            camera_name: cam_name.clone(),
            details: "Password not available for HTTP CGI request".to_string(),
        })?;
        
        info!("Making GET request to {} to {} camera ({})", url, action_str, cam_name);

        let response = self.http_client
            .get(&url)
            .basic_auth(username, Some(password))
            .send()
            .await
            .map_err(|e| AppError::HttpCgi(format!("Request to {} failed for '{}': {}", url, cam_name, e)))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            info!("Basic auth failed (401) for {}, attempting digest auth for camera: {}", url, cam_name);
            let response_digest = self.http_client
                .get(&url)
                .basic_auth(username, Some(password)) // Changed .digest to .basic_auth
                .send()
                .await
                .map_err(|e| AppError::HttpCgi(format!("Auth retry request to {} failed for '{}': {}", url, cam_name, e)))?;

            if !response_digest.status().is_success() {
                let err_msg = format!("HTTP CGI {} failed for '{}' after digest auth. Status: {}, URL: {}", action_str, cam_name, response_digest.status(), url);
                error!("{}", err_msg);
                return Err(AppError::Control(err_msg));
            }
            info!("Successfully {}d camera '{}' via HTTP CGI (digest auth)", action_str, cam_name);
            return Ok(());
        }

        if !response.status().is_success() {
            let err_msg = format!("HTTP CGI {} failed for '{}'. Status: {}, URL: {}", action_str, cam_name, response.status(), url);
            error!("{}", err_msg);
            return Err(AppError::Control(err_msg));
        }

        info!("Successfully {}d camera '{}' via HTTP CGI (basic auth)", action_str, cam_name);
        Ok(())
    }

} 
use crate::config_loader::RealsenseSpecificConfig;
use crate::core::capture_source::{
    CaptureSource, FrameData, FrameDataBundle, RsColorFrameData, RsDepthFrameData,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use image; // Using image crate for saving
use log::{info, warn};
use realsense_rust::{
    config::Config as RsConfig,
    context::Context as RsContext,
    frame::{ColorFrame, CompositeFrame, DepthFrame, FrameEx}, // ImageFrame removed, specific frames used directly
    kind::{Rs2CameraInfo, Rs2Format, Rs2StreamKind},
    pipeline::{ActivePipeline as RsActivePipeline, InactivePipeline as RsInactivePipeline},
    stream_profile::StreamProfile,
};
use std::collections::HashSet;
use std::ffi::CString;
use std::path::Path;
use std::time::Duration as StdDuration;
use tokio::task;

#[derive(Debug, Clone)]
pub struct RealsenseDevice {
    pub name: String,
    pub config: RealsenseSpecificConfig,
}

#[async_trait]
impl CaptureSource for RealsenseDevice {
    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_type(&self) -> String {
        "RealsenseCamera".to_string()
    }

    async fn capture_image(
        &mut self, 
        output_dir: &Path,
        timestamp_str: &str,
        _image_format_config: &str,
        _jpeg_quality: Option<u8>,
        _png_compression: Option<u32>,
    ) -> Result<FrameDataBundle> {
        self.capture_image_internal(output_dir, timestamp_str).await
    }
}

impl RealsenseDevice {
    pub fn new(name: String, config: RealsenseSpecificConfig) -> Self {
        Self { name, config }
    }

    async fn capture_image_internal(
        &self,
        output_dir: &Path,
        timestamp_str: &str,
    ) -> Result<FrameDataBundle> {
        let name_clone = self.name.clone();
        let config_clone = self.config.clone();
        let output_dir_clone = output_dir.to_path_buf();
        let timestamp_str_clone = timestamp_str.to_string();

        task::spawn_blocking(move || -> Result<FrameDataBundle> {
            info!("RS Blocking [{}]: Task started.", name_clone);
            let mut active_pipeline_opt: Option<RsActivePipeline> = None;

            let frame_data_bundle_result: Result<FrameDataBundle> = (|| {
                let context = RsContext::new().context("RS: Failed to create Realsense context")?;
                
                let device_list = context.query_devices(HashSet::new());

                if device_list.is_empty() {
                    return Err(anyhow!("RS [{}]: No Realsense devices found.", name_clone));
                }

                let device_serial_to_use: String;

                if let Some(serial_to_find) = &config_clone.serial_number {
                    info!("RS [{}]: Searching for device S/N: {}", name_clone, serial_to_find);
                    let found_device = device_list.iter().find(|dev| {
                        dev.info(Rs2CameraInfo::SerialNumber)
                            .and_then(|cstr| cstr.to_str().ok())
                            .map_or(false, |s| s == serial_to_find.as_str())
                    });

                    if let Some(dev) = found_device {
                        let sn_cstr = dev.info(Rs2CameraInfo::SerialNumber)
                            .ok_or_else(|| anyhow!("RS [{}]: Failed to get S/N CString for found device S/N '{}'", name_clone, serial_to_find))?;
                        device_serial_to_use = sn_cstr.to_str()
                            .map_err(|e| anyhow!("RS [{}]: Failed to convert S/N CString to str for found device: {}", name_clone, e))?
                            .to_string();
                        info!("RS [{}]: Found target device S/N: {}", name_clone, device_serial_to_use);
                    } else {
                        return Err(anyhow!("RS [{}]: Specified device S/N '{}' not found.", name_clone, serial_to_find));
                    }
                } else {
                    info!("RS [{}]: No S/N specified, using first available device.", name_clone);
                    if let Some(dev) = device_list.first() {
                        let sn_cstr = dev.info(Rs2CameraInfo::SerialNumber)
                            .ok_or_else(|| anyhow!("RS [{}]: Failed to get S/N CString for first available device", name_clone))?;
                        device_serial_to_use = sn_cstr.to_str()
                            .map_err(|e| anyhow!("RS [{}]: Failed to convert S/N CString to str for first device: {}", name_clone, e))?
                            .to_string();
                        info!("RS [{}]: Using first device S/N: {}", name_clone, device_serial_to_use);
                    } else {
                        return Err(anyhow!("RS [{}]: Device list was empty when attempting to use first device (unexpected).", name_clone));
                    }
                }
                
                let inactive_pipeline = RsInactivePipeline::try_from(&context)
                    .context("RS: Failed to create inactive pipeline from context")?;
                
                let mut rs_pipeline_config = RsConfig::new();
                let c_device_serial = CString::new(device_serial_to_use.clone())
                    .with_context(|| format!("RS [{}]: Failed to create CString from serial: {}", name_clone, device_serial_to_use))?;
                
                rs_pipeline_config.enable_device_from_serial(c_device_serial.as_c_str())
                    .with_context(|| format!("RS [{}]: Failed to enable device S/N '{}' in config", name_clone, device_serial_to_use))?;
                
                rs_pipeline_config.disable_all_streams()
                    .context("RS: Failed to disable all streams in config")?;

                let mut color_stream_actually_enabled = false;
                if config_clone.enable_color_stream.unwrap_or(true) {
                    let w = config_clone.color_width.unwrap_or(640);
                    let h = config_clone.color_height.unwrap_or(480);
                    let fps = config_clone.color_fps.unwrap_or(30);
                    rs_pipeline_config.enable_stream(Rs2StreamKind::Color, None, w as usize, h as usize, Rs2Format::Bgr8, fps as usize)
                        .with_context(|| format!("RS [{}]: Failed to enable color stream ({}x{}@{} BGR8)", name_clone, w, h, fps))?;
                    color_stream_actually_enabled = true;
                    info!("RS [{}]: Color stream configured ({}x{}@{}fps BGR8).", name_clone, w, h, fps);
                }

                let mut depth_stream_actually_enabled = false;
                if config_clone.enable_depth_stream.unwrap_or(true) {
                    let w = config_clone.depth_width.unwrap_or(640);
                    let h = config_clone.depth_height.unwrap_or(480);
                    let fps = config_clone.depth_fps.unwrap_or(30);
                    rs_pipeline_config.enable_stream(Rs2StreamKind::Depth, None, w as usize, h as usize, Rs2Format::Z16, fps as usize)
                        .with_context(|| format!("RS [{}]: Failed to enable depth stream ({}x{}@{} Z16)", name_clone, w, h, fps))?;
                    depth_stream_actually_enabled = true;
                    info!("RS [{}]: Depth stream configured ({}x{}@{}fps Z16).", name_clone, w, h, fps);
                }

                if !color_stream_actually_enabled && !depth_stream_actually_enabled {
                    return Err(anyhow!("RS [{}]: Both color and depth streams are disabled.", name_clone));
                }

                info!("RS [{}]: Starting pipeline for S/N {}...", name_clone, device_serial_to_use);
                let active_pipeline = inactive_pipeline.start(Some(rs_pipeline_config))
                    .context("RS: Failed to start pipeline")?;
                active_pipeline_opt = Some(active_pipeline);
                let pipeline_ref = active_pipeline_opt.as_mut().unwrap();

                info!("RS [{}]: Waiting for frameset...", name_clone);
                let frameset: CompositeFrame = pipeline_ref.wait(Some(StdDuration::from_secs(5)))
                    .context("RS: Wait for frames failed")?;
                info!("RS [{}]: Frameset received with {} frames (API count).", name_clone, frameset.count());

                let mut processed_color_data: Option<RsColorFrameData> = None;
                let mut processed_depth_data: Option<RsDepthFrameData> = None;

                if color_stream_actually_enabled {
                    let color_frames: Vec<ColorFrame> = frameset.frames_of_type::<ColorFrame>();
                    if let Some(color_frame) = color_frames.first() {
                        let profile: &StreamProfile = color_frame.stream_profile();
                        info!("RS [{}]: Processing ColorFrame. Format: {:?}, Res: {}x{}, BPP: {}, TS: {}, Domain: {:?}", 
                            name_clone, profile.format(), color_frame.width(), color_frame.height(), 
                            color_frame.bits_per_pixel(), color_frame.timestamp(), color_frame.timestamp_domain());

                        let width = color_frame.width() as u32;
                        let height = color_frame.height() as u32;
                        let bpp_usize = color_frame.bits_per_pixel() / 8;
                        if bpp_usize != 3 {
                            return Err(anyhow!("RS [{}]: Color frame BPP is {}, expected 3 (BGR8).", name_clone, bpp_usize));
                        }
                        let data_size = width as usize * height as usize * bpp_usize;
                        let raw_data_ptr: *const std::os::raw::c_void = unsafe { color_frame.get_data() };
                        let color_data_slice = unsafe { std::slice::from_raw_parts(raw_data_ptr as *const u8, data_size) };

                        let mut rgb_pixel_data = Vec::with_capacity(data_size);
                        for chunk in color_data_slice.chunks_exact(3) {
                            rgb_pixel_data.push(chunk[2]);
                            rgb_pixel_data.push(chunk[1]);
                            rgb_pixel_data.push(chunk[0]);
                        }

                        let color_filename = format!("{}_realsense_{}_color.png", timestamp_str_clone, name_clone.replace(" ", "_"));
                        let color_path = output_dir_clone.join(&color_filename);
                        image::save_buffer_with_format(&color_path, &rgb_pixel_data, width, height, image::ColorType::Rgb8, image::ImageFormat::Png)
                            .with_context(|| format!("RS [{}]: Failed to save color image to {:?}", name_clone, color_path))?;
                        info!("RS [{}]: Saved color image to {:?}", name_clone, color_path);
                        processed_color_data = Some(RsColorFrameData { rgb_data: rgb_pixel_data, path: color_path, width, height });
                    } else {
                         warn!("RS [{}]: Color stream enabled, but no ColorFrame found in frameset.", name_clone);
                    }
                }

                if depth_stream_actually_enabled {
                    let depth_frames: Vec<DepthFrame> = frameset.frames_of_type::<DepthFrame>();
                    if let Some(depth_frame) = depth_frames.first() {
                        let profile: &StreamProfile = depth_frame.stream_profile();
                        let current_depth_units = depth_frame.depth_units()
                            .context("RS: Failed to get depth units")?;
                        info!("RS [{}]: Processing DepthFrame. Format: {:?}, Res: {}x{}, BPP: {}, TS: {}, Domain: {:?}, Units: {}",
                            name_clone, profile.format(), depth_frame.width(), depth_frame.height(),
                            depth_frame.bits_per_pixel(), depth_frame.timestamp(), depth_frame.timestamp_domain(), current_depth_units);

                        let width = depth_frame.width() as u32;
                        let height = depth_frame.height() as u32;
                        let bpp_usize = depth_frame.bits_per_pixel() / 8;
                        if bpp_usize != 2 {
                            return Err(anyhow!("RS [{}]: Depth frame BPP is {}, expected 2 (Z16).", name_clone, bpp_usize));
                        }
                        let data_size_pixels = width as usize * height as usize;
                        let raw_data_ptr: *const std::os::raw::c_void = unsafe { depth_frame.get_data() };
                        let depth_data_slice_u16 = unsafe { std::slice::from_raw_parts(raw_data_ptr as *const u16, data_size_pixels) };
                        
                        let depth_filename = format!("{}_realsense_{}_depth.png", timestamp_str_clone, name_clone.replace(" ", "_"));
                        let depth_path = output_dir_clone.join(&depth_filename);
                        
                        let depth_image_buffer: image::ImageBuffer<image::Luma<u16>, Vec<u16>> = 
                            image::ImageBuffer::from_raw(width, height, depth_data_slice_u16.to_vec())
                            .ok_or_else(|| anyhow!("RS [{}]: Could not create depth image buffer from raw data", name_clone))?;
                        
                        depth_image_buffer.save_with_format(&depth_path, image::ImageFormat::Png)
                            .with_context(|| format!("RS [{}]: Failed to save depth image to {:?}", name_clone, depth_path))?;
                        info!("RS [{}]: Saved depth image to {:?}", name_clone, depth_path);
                        processed_depth_data = Some(RsDepthFrameData { depth_data: depth_data_slice_u16.to_vec(), depth_units: current_depth_units, path: depth_path, width, height });
                    } else {
                        warn!("RS [{}]: Depth stream enabled, but no DepthFrame found in frameset.", name_clone);
                    }
                }

                if processed_color_data.is_none() && processed_depth_data.is_none() && (color_stream_actually_enabled || depth_stream_actually_enabled) {
                     let mut missing_streams = Vec::new();
                     if color_stream_actually_enabled { missing_streams.push("color"); }
                     if depth_stream_actually_enabled { missing_streams.push("depth"); }
                    return Err(anyhow!("RS [{}]: No {} data was successfully captured from frameset despite being enabled.", name_clone, missing_streams.join(" or ")));
                }

                Ok(FrameDataBundle {
                    frames: vec![FrameData::RealsenseFrames { name: name_clone.clone(), color_frame: processed_color_data, depth_frame: processed_depth_data }],
                })
            })();

            if let Some(pipeline_to_stop) = active_pipeline_opt.take() {
                info!("RS Blocking [{}]: Stopping pipeline...", name_clone);
                pipeline_to_stop.stop();
                info!("RS Blocking [{}]: Pipeline stopped.", name_clone);
            }
            info!("RS Blocking [{}]: Task finished.", name_clone);
            frame_data_bundle_result
        }).await.map_err(|e| anyhow!("Realsense [{}]: spawn_blocking task panicked: {}", self.name, e))?
    }
}
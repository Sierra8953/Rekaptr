use crate::video_player::video::Video;
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
use gstreamer as gst;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;
use std::sync::Arc;
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_bgra};

pub struct VideoElement {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
    element_id: Option<ElementId>,
}

impl VideoElement {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
            element_id: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    pub fn size(mut self, width: gpui::Pixels, height: gpui::Pixels) -> Self {
        self.display_width = Some(width);
        self.display_height = Some(height);
        self
    }

    pub fn buffer_capacity(self, capacity: usize) -> Self {
        self.video.set_frame_buffer_capacity(capacity);
        self
    }

    fn get_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        match (self.display_width, self.display_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let (w, h) = self.video.display_size();
                (gpui::px(w as f32), gpui::px(h as f32))
            }
        }
    }

    fn fitted_bounds(
        &self,
        bounds: gpui::Bounds<gpui::Pixels>,
        frame_width: u32,
        frame_height: u32,
    ) -> gpui::Bounds<gpui::Pixels> {
        let container_w: f32 = bounds.size.width.into();
        let container_h: f32 = bounds.size.height.into();
        let frame_w = frame_width as f32;
        let frame_h = frame_height as f32;

        let scale = if frame_w > 0.0 && frame_h > 0.0 {
            (container_w / frame_w).min(container_h / frame_h)
        } else {
            1.0
        };

        let dest_w = frame_w * scale;
        let dest_h = frame_h * scale;
        
        let offset_x = ((container_w - dest_w) / 2.0).max(0.0);
        let offset_y = ((container_h - dest_h) / 2.0).max(0.0);

        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px(offset_x),
                bounds.origin.y + gpui::px(offset_y),
            ),
            gpui::size(gpui::px(dest_w), gpui::px(dest_h)),
        )
    }

    fn paint_render_image(
        &mut self,
        window: &mut Window,
        _cx: &mut gpui::App,
        bounds: gpui::Bounds<gpui::Pixels>,
        rgb_data: Vec<u8>,
        frame_width: u32,
        frame_height: u32,
    ) {
        use image::{ImageBuffer, Rgba};

        if let Some(image_buffer) =
            ImageBuffer::<Rgba<u8>, _>::from_raw(frame_width, frame_height, rgb_data)
        {
            let render_frame = image::Frame::new(image_buffer);
            let render_image = Arc::new(gpui::RenderImage::new(vec![render_frame]));

            let dest_bounds = self.fitted_bounds(bounds, frame_width, frame_height);

            window
                .paint_image(
                    dest_bounds,
                    gpui::Corners::default(),
                    render_image,
                    0,
                    false,
                )
                .ok();
        }
    }
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = gpui::Style {
            size: gpui::Size {
                width: gpui::relative(1.0).into(),
                height: gpui::relative(1.0).into(),
            },
            ..Default::default()
        };

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        let is_playing = !self.video.paused();
        let has_new_frame = self.video.take_frame_ready();
        if is_playing || has_new_frame {
            window.request_animation_frame();
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _prepaint_state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        // Fill background with black
        window.paint_quad(gpui::fill(bounds, gpui::rgb(0x000000)));

        let buffered = self.video.buffered_len();
        let mut frame_to_render: Option<gst::Sample> = None;
        if buffered > 0 {
            let inner = self.video.0.read();
            let mut buf = inner.frame_buffer.lock();
            // Pop only the oldest frame if we are playing, or the latest if we are catching up
            // For now, popping one frame per paint call is better than popping all of them
            if let Some(frame) = buf.pop_front() {
                frame_to_render = Some(frame.0);
            }
        } else {
            let inner = self.video.0.read();
            let sample = inner.frame.lock().0.clone();
            if sample.buffer().is_some() {
                frame_to_render = Some(sample);
            }
        }

        if let Some(sample) = frame_to_render {
            let info = self.video.0.read().info.read().clone();
            let frame_width = info.width();
            let frame_height = info.height();
            
            if frame_width == 0 || frame_height == 0 {
                return;
            }

            if let Some(buffer) = sample.buffer() {
                if let Ok(frame) = gst_video::VideoFrame::from_buffer_readable(buffer.to_owned(), &info) {
                    let mut bgra = vec![0u8; (frame_width * frame_height * 4) as usize];
                    let rgba_stride = frame_width * 4;
                    
                    let y_plane = frame.plane_data(0);
                    let uv_plane = frame.plane_data(1);
                    let strides = frame.plane_stride();

                    if y_plane.is_ok() && uv_plane.is_ok() {
                        if strides.len() >= 2 {
                            let y = y_plane.unwrap();
                            let uv = uv_plane.unwrap();
                            let y_stride = strides[0] as u32;
                            let uv_stride = strides[1] as u32;

                            let yuv_bi_planar = YuvBiPlanarImage {
                                y_plane: y,
                                y_stride,
                                uv_plane: uv,
                                uv_stride,
                                width: frame_width,
                                height: frame_height,
                            };

                            let conv_result = yuv_nv12_to_bgra(
                                &yuv_bi_planar,
                                &mut bgra,
                                rgba_stride,
                                YuvRange::Full,
                                YuvStandardMatrix::Bt709,
                                YuvConversionMode::Balanced,
                            );

                            if conv_result.is_ok() {
                                self.paint_render_image(window, cx, bounds, bgra, frame_width, frame_height);
                            } else {
                                // Try limited range if full fails
                                let conv_result_limited = yuv_nv12_to_bgra(
                                    &yuv_bi_planar,
                                    &mut bgra,
                                    rgba_stride,
                                    YuvRange::Limited,
                                    YuvStandardMatrix::Bt709,
                                    YuvConversionMode::Balanced,
                                );
                                
                                if conv_result_limited.is_ok() {
                                    self.paint_render_image(window, cx, bounds, bgra, frame_width, frame_height);
                                } else {
                                    eprintln!("[Video] YUV to BGRA conversion failed: {:?}", conv_result_limited.err());
                                }
                            }
                        } else {
                            eprintln!("[Video] Invalid strides count: {}", strides.len());
                        }
                    } else {
                        eprintln!("[Video] Failed to get plane data (Y: {:?}, UV: {:?})", y_plane.is_err(), uv_plane.is_err());
                    }
                } else {
                    // This can happen if the info doesn't match the buffer size
                    // We'll skip this frame and wait for the next one or for info to update
                }
            }
        }
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub fn video(video: Video) -> VideoElement {
    VideoElement::new(video)
}

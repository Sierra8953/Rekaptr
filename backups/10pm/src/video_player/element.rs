use crate::video_player::video::Video;
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
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

        // Important: Calculate destination size strictly based on the scale and frame, 
        // DO NOT allow offset_x or offset_y to be negative (which pushes the frame out of bounds)
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
        cx: &mut gpui::App,
        bounds: gpui::Bounds<gpui::Pixels>,
        rgb_data: Vec<u8>,
        frame_width: u32,
        frame_height: u32,
    ) {
        use image::{ImageBuffer, Rgba};

        if let Some(image_buffer) =
            ImageBuffer::<Rgba<u8>, _>::from_raw(frame_width, frame_height, rgb_data)
        {
            let last_render_image: gpui::Entity<Option<Arc<gpui::RenderImage>>> =
                window.use_state(cx, |_, _| None);

            let render_frame = image::Frame::new(image_buffer);
            // Use Vec which should implement Into<SmallVec<...>>
            let render_image = Arc::new(gpui::RenderImage::new(vec![render_frame]));

            let dest_bounds = self.fitted_bounds(bounds, frame_width, frame_height);

            let prev_image: Option<Arc<gpui::RenderImage>> =
                last_render_image.update(cx, |this, _| this.replace(render_image.clone()));

            window
                .paint_image(
                    dest_bounds,
                    gpui::Corners::default(),
                    render_image.clone(),
                    0,
                    false,
                )
                .ok();

            if let Some(prev) = prev_image {
                cx.drop_image(prev, Some(window));
            }
        }
    }

    fn yuv_to_rgb(&self, yuv_data: &[u8], width: u32, height: u32) -> Vec<u8> {
        let width_usize = width as usize;
        let height_usize = height as usize;
        let y_size = width_usize * height_usize;
        let uv_size = (width_usize * height_usize) / 2;

        if yuv_data.len() < y_size + uv_size {
            return vec![0; width_usize * height_usize * 4];
        }

        let y_plane = &yuv_data[..y_size];
        let uv_plane = &yuv_data[y_size..y_size + uv_size];

        let yuv_bi_planar = YuvBiPlanarImage {
            y_plane,
            y_stride: width,
            uv_plane,
            uv_stride: width,
            width,
            height,
        };

        let mut bgra = vec![0u8; width_usize * height_usize * 4];
        let rgba_stride = width * 4;

        if yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Full,
            YuvStandardMatrix::Bt709,
            YuvConversionMode::Balanced,
        )
        .is_ok()
        {
            return bgra;
        }

        if yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Limited,
            YuvStandardMatrix::Bt709,
            YuvConversionMode::Balanced,
        )
        .is_ok()
        {
            return bgra;
        }

        match yuv_nv12_to_bgra(
            &yuv_bi_planar,
            &mut bgra,
            rgba_stride,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Balanced,
        ) {
            Ok(_) => bgra,
            Err(_) => {
                vec![0; width_usize * height_usize * 4]
            }
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
                width: gpui::Length::Auto,
                height: gpui::Length::Auto,
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
        let buffered = self.video.buffered_len();
        let mut frame_to_render: Option<(Vec<u8>, u32, u32)> = None;
        if buffered > 0 {
            for _ in 0..buffered {
                if let Some(frame) = self.video.pop_buffered_frame() {
                    frame_to_render = Some(frame);
                }
            }
        } else {
            frame_to_render = self.video.current_frame_data();
        }

        if let Some((yuv_data, frame_width, frame_height)) = frame_to_render {
            let rgb_data = self.yuv_to_rgb(&yuv_data, frame_width, frame_height);
            self.paint_render_image(window, cx, bounds, rgb_data, frame_width, frame_height);
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

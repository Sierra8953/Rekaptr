use crate::video_player::video::Video;
use windows::core::Interface;
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};

pub struct VideoElement {
    video: Video,
    element_id: Option<ElementId>,
}

impl VideoElement {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            element_id: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    pub fn buffer_capacity(self, _capacity: usize) -> Self {
        self
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
        // Trigger MPV to render the next frame into the shared texture
        if self.video.take_frame_ready() || !self.video.paused() {
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
        _cx: &mut gpui::App,
    ) {
        let inner = self.video.read();
        if inner.width > 0 && inner.height > 0 {
            let (vw, vh) = self.video.display_size();
            let dest_bounds = self.fitted_bounds(bounds, vw, vh);

            window
                .paint_hardware_texture(
                    dest_bounds,
                    gpui::Corners::default(),
                    inner.render_image.clone(),
                    0,
                    gpui::size(inner.width.into(), inner.height.into()),
                    inner.d3d_texture.as_raw(),
                )
                .ok();
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

use crate::render_system::RenderSystem;
use std::sync::Arc;
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::swapchain::{Surface, Swapchain, SwapchainCreateInfo};
use winit::window::Window;
use crate::render_device::RenderDevice;

pub struct RenderOutput {
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
}

impl RenderOutput {
    pub fn new(
        _render_system: &RenderSystem,
        render_device: &RenderDevice,
        surface: &Arc<Surface>,
    ) -> Self {
        let (swapchain, images) = {
            let surface_capabilities = render_device
                .device
                .physical_device()
                .surface_capabilities(surface, Default::default())
                .unwrap();
            let image_format = Some(
                render_device
                    .device
                    .physical_device()
                    .surface_formats(surface, Default::default())
                    .unwrap()[0]
                    .0,
            );
            let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();
            Swapchain::new(
                render_device.device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,
                    image_format,
                    image_extent: window.inner_size().into(),

                    image_usage: ImageUsage {
                        color_attachment: true,
                        ..ImageUsage::empty()
                    },
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .iter()
                        .next()
                        .unwrap(),
                    ..Default::default()
                },
            )
            .unwrap()
        };
        Self { swapchain, images }
    }
}

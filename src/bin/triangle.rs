use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        RenderingAttachmentInfo, RenderingInfo,
    },
    image::{ImageAccess, SwapchainImage, view::ImageView},
    impl_vertex,
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            render_pass::PipelineRenderingCreateInfo,
            vertex_input::BuffersDefinition,
            viewport::{Viewport, ViewportState},
        },
        GraphicsPipeline,
    },
    render_pass::{LoadOp, StoreOp},
    swapchain::{
        acquire_next_image, AcquireError, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use renderer::render_output::RenderOutput;
use renderer::render_system::{RenderDevice, RenderSystem};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
struct Vertex {
    position: [f32; 2],
}
impl_vertex!(Vertex, position);

fn main() {
    let render_system = RenderSystem::new();
    let event_loop = EventLoop::new();
    let surface = WindowBuilder::new()
        .build_vk_surface(&event_loop, render_system.instance.clone())
        .unwrap();
    let render_device = RenderDevice::new(&render_system, &surface);
    let mut render_output = RenderOutput::new(&render_system, &render_device, &surface);

    let vertices = [
        Vertex {
            position: [-0.5, -0.25],
        },
        Vertex {
            position: [0.0, 0.5],
        },
        Vertex {
            position: [0.25, -0.1],
        },
    ];
    let vertex_buffer = CpuAccessibleBuffer::from_iter(
        &render_device.memory_allocator,
        BufferUsage {
            vertex_buffer: true,
            ..BufferUsage::empty()
        },
        false,
        vertices,
    )
    .unwrap();

    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            src: "
				#version 450

				layout(location = 0) in vec2 position;

				void main() {
					gl_Position = vec4(position, 0.0, 1.0);
				}
			"
        }
    }

    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            src: "
				#version 450

				layout(location = 0) out vec4 f_color;

				void main() {
					f_color = vec4(1.0, 0.0, 0.0, 1.0);
				}
			"
        }
    }

    let vs = vs::load(render_device.device.clone()).unwrap();
    let fs = fs::load(render_device.device.clone()).unwrap();

    let pipeline = GraphicsPipeline::start()
        // We describe the formats of attachment images where the colors, depth and/or stencil
        // information will be written. The pipeline will only be usable with this particular
        // configuration of the attachment images.
        .render_pass(PipelineRenderingCreateInfo {
            // We specify a single color attachment that will be rendered to. When we begin
            // rendering, we will specify a swapchain image to be used as this attachment, so here
            // we set its format to be the same format as the swapchain.
            color_attachment_formats: vec![Some(render_output.swapchain.image_format())],
            ..Default::default()
        })
        // We need to indicate the layout of the vertices.
        .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
        // The content of the vertex buffer describes a list of triangles.
        .input_assembly_state(InputAssemblyState::new())
        // A Vulkan shader can in theory contain multiple entry points, so we have to specify
        // which one.
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        // Use a resizable viewport set to draw over the entire window
        .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        // See `vertex_shader`.
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        // Now that our builder is filled, we call `build()` to obtain an actual pipeline.
        .build(render_device.device.clone())
        .unwrap();

    // Dynamic viewports allow us to recreate just the viewport when the window is resized
    // Otherwise we would have to recreate the whole pipeline.
    let mut viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [0.0, 0.0],
        depth_range: 0.0..1.0,
    };

    // When creating the swapchain, we only created plain images. To use them as an attachment for
    // rendering, we must wrap then in an image view.
    //
    // Since we need to draw to multiple images, we are going to create a different image view for
    // each image.
    let mut attachment_image_views =
        window_size_dependent_setup(&render_output.images, &mut viewport);

    // Before we can start creating and recording command buffers, we need a way of allocating
    // them. Vulkano provides a command buffer allocator, which manages raw Vulkan command pools
    // underneath and provides a safe interface for them.
    let command_buffer_allocator =
        StandardCommandBufferAllocator::new(render_device.device.clone(), Default::default());

    // Initialization is finally finished!

    // In some situations, the swapchain will become invalid by itself. This includes for example
    // when the window is resized (as the images of the swapchain will no longer match the
    // window's) or, on Android, when the application went to the background and goes back to the
    // foreground.
    //
    // In this situation, acquiring a swapchain image or presenting it will return an error.
    // Rendering to an image of that swapchain will not produce any error, but may or may not work.
    // To continue rendering, we need to recreate the swapchain by creating a new swapchain.
    // Here, we remember that we need to do this for the next loop iteration.
    let mut recreate_swapchain = false;

    // In the loop below we are going to submit commands to the GPU. Submitting a command produces
    // an object that implements the `GpuFuture` trait, which holds the resources for as long as
    // they are in use by the GPU.
    //
    // Destroying the `GpuFuture` blocks until the GPU is finished executing it. In order to avoid
    // that, we store the submission of the previous frame here.
    let mut previous_frame_end = Some(sync::now(render_device.device.clone()).boxed());

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                recreate_swapchain = true;
            }
            Event::RedrawEventsCleared => {
                previous_frame_end.as_mut().unwrap().cleanup_finished();
                if recreate_swapchain {
                    // Get the new dimensions of the window.
                    let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();

                    let (new_swapchain, new_images) =
                        match render_output.swapchain.recreate(SwapchainCreateInfo {
                            image_extent: window.inner_size().into(),
                            ..render_output.swapchain.create_info()
                        }) {
                            Ok(r) => r,
                            Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                        };

                    render_output.swapchain = new_swapchain;
                    attachment_image_views =
                        window_size_dependent_setup(&new_images, &mut viewport);
                    recreate_swapchain = false;
                }

                let (image_index, suboptimal, acquire_future) =
                    match acquire_next_image(render_output.swapchain.clone(), None) {
                        Ok(r) => r,
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    };
                if suboptimal {
                    recreate_swapchain = true;
                }
                let mut builder = AutoCommandBufferBuilder::primary(
                    &command_buffer_allocator,
                    render_device.present_queue.queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                )
                .unwrap();

                builder
                    .begin_rendering(RenderingInfo {
                        color_attachments: vec![Some(RenderingAttachmentInfo {
                            load_op: LoadOp::Clear,
                            store_op: StoreOp::Store,
                            clear_value: Some([0.0, 0.0, 1.0, 1.0].into()),
                            ..RenderingAttachmentInfo::image_view(
                                attachment_image_views[image_index as usize].clone(),
                            )
                        })],
                        ..Default::default()
                    })
                    .unwrap()
                    .set_viewport(0, [viewport.clone()])
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_vertex_buffers(0, vertex_buffer.clone())
                    .draw(vertex_buffer.len() as u32, 1, 0, 0)
                    .unwrap()
                    .end_rendering()
                    .unwrap();
                let command_buffer = builder.build().unwrap();

                let future = previous_frame_end
                    .take()
                    .unwrap()
                    .join(acquire_future)
                    .then_execute(render_device.present_queue.clone(), command_buffer)
                    .unwrap()
                    .then_swapchain_present(
                        render_device.present_queue.clone(),
                        SwapchainPresentInfo::swapchain_image_index(
                            render_output.swapchain.clone(),
                            image_index,
                        ),
                    )
                    .then_signal_fence_and_flush();

                match future {
                    Ok(future) => {
                        previous_frame_end = Some(future.boxed());
                    }
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        previous_frame_end = Some(sync::now(render_device.device.clone()).boxed());
                    }
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        previous_frame_end = Some(sync::now(render_device.device.clone()).boxed());
                    }
                }
            }
            _ => (),
        }
    });
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage>],
    viewport: &mut Viewport,
) -> Vec<Arc<ImageView<SwapchainImage>>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    images
        .iter()
        .map(|image| ImageView::new_default(image.clone()).unwrap())
        .collect::<Vec<_>>()
}

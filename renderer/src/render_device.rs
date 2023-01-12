use std::sync::Arc;
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo};
use vulkano::device::physical::PhysicalDeviceType;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::Surface;
use vulkano::Version;
use crate::render_system::RenderSystem;

pub struct RenderDevice {
    pub device: Arc<Device>,
    pub memory_allocator: StandardMemoryAllocator,
    /// One queue to present to swapchain. Supports graphics and compute.
    pub present_queue: Arc<Queue>,
}

impl RenderDevice {
    pub fn new(system: &RenderSystem, surface: &Arc<Surface>) -> Self {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            khr_dynamic_rendering: true,
            // ext_descriptor_indexing: true,
            ..DeviceExtensions::empty()
        };
        let (physical_device, queue_family_index) = system
            .instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| p.api_version() >= Version::V1_2)
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.graphics
                            && p.surface_support(i as u32, surface).unwrap_or(false)
                    })
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| {
                // We assign a lower score to device types that are likely to be faster/better.
                match p.properties().device_type {
                    PhysicalDeviceType::DiscreteGpu => 0,
                    PhysicalDeviceType::IntegratedGpu => 1,
                    PhysicalDeviceType::VirtualGpu => 2,
                    PhysicalDeviceType::Cpu => 3,
                    PhysicalDeviceType::Other => 4,
                    _ => 5,
                }
            })
            .expect("No suitable physical device found");
        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );
        let (device, mut queues) = Device::new(
            // Which physical device to connect to.
            physical_device,
            DeviceCreateInfo {
                enabled_extensions: device_extensions,
                enabled_features: Features {
                    dynamic_rendering: true,
                    ..Features::empty()
                },

                // The list of queues that we are going to use. Here we only use one queue, from the
                // previously chosen queue family.
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],

                ..Default::default()
            },
        )
            .unwrap();
        let queue = queues.next().unwrap();
        let memory_allocator = StandardMemoryAllocator::new_default(device.clone());

        Self {
            device,
            memory_allocator,
            present_queue: queue,
        }
    }
}

use std::sync::Arc;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo,
};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::swapchain::Surface;
use vulkano::*;
use vulkano::instance::debug::{DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger, DebugUtilsMessengerCreateInfo};

/// A basic, single-device render system
pub struct RenderSystem {
    pub instance: Arc<Instance>,
    pub physical_devices: Vec<Arc<PhysicalDevice>>,
    pub debug_callback: Option<DebugUtilsMessenger>,
}

impl RenderSystem {
    pub fn new() -> Self {
        let library = VulkanLibrary::new().unwrap();
        let required_extensions = InstanceExtensions {
            ext_debug_utils: true,
            ..vulkano_win::required_extensions(&library)
        };
        //  TODO: more choices
        let layers = vec!["VK_LAYER_KHRONOS_validation".to_owned()];
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: required_extensions,
                enabled_layers: layers,
                enumerate_portability: cfg!(target_os = "macos"),
                ..Default::default()
            },
        )
            .unwrap();
        let debug_callback = unsafe {
            DebugUtilsMessenger::new(
                instance.clone(),
                DebugUtilsMessengerCreateInfo {
                    message_severity: DebugUtilsMessageSeverity {
                        error: true,
                        warning: true,
                        information: true,
                        verbose: true,
                        ..DebugUtilsMessageSeverity::empty()
                    },
                    message_type: DebugUtilsMessageType {
                        general: true,
                        validation: true,
                        performance: true,
                        ..DebugUtilsMessageType::empty()
                    },
                    ..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
                        let severity = if msg.severity.error {
                            "ERROR"
                        } else if msg.severity.warning {
                            "WARN"
                        } else if msg.severity.information {
                            "INFO"
                        } else if msg.severity.verbose {
                            "VERBOSE"
                        } else {
                            panic!("no-impl");
                        };

                        let ty = if msg.ty.general {
                            "General"
                        } else if msg.ty.validation {
                            "Validation"
                        } else if msg.ty.performance {
                            "Performance"
                        } else {
                            panic!("no-impl");
                        };

                        println!(
                            "{} [{}] ({}): {}",
                            severity,
                            ty,
                            msg.layer_prefix.unwrap_or("Unknown"),
                            msg.description
                        );
                    }))
                },
            )
                .ok()
        };

        let physical_devices = instance.enumerate_physical_devices().unwrap().collect();

        Self {
            instance,
            physical_devices,
            debug_callback,
        }
    }
}


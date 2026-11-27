//! Minimal Vulkan bring-up for the Windows executable running under Wine/Proton.
//!
//! This deliberately does not submit game work yet. It verifies that the
//! Windows Vulkan loader exposed by Proton can create an instance, select a
//! physical device, and create a graphics+compute queue. The existing D3D12
//! host remains active until resource, command, pipeline and presentation
//! translation have been ported.

use ash::{Entry, vk};
use std::ffi::CStr;

/// Load Vulkan through `vulkan-1.dll` (provided by Proton) and create a
/// temporary logical device. Returns a diagnostic error rather than
/// preventing the existing D3D12 path from starting.
pub(crate) fn probe() -> Result<(), String> {
    // SAFETY: ash loads the system Vulkan loader; the returned Entry owns
    // the dynamic library for the lifetime of all Vulkan calls below.
    let entry = unsafe { Entry::load() }
        .map_err(|error| format!("could not load Vulkan loader: {error}"))?;

    let app_name = c"Victory";
    let engine_name = c"Victory GPU";
    let app_info = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(engine_name)
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::make_api_version(0, 1, 2, 0));
    let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);

    // SAFETY: create info and application strings live through the call.
    let instance = unsafe { entry.create_instance(&instance_info, None) }
        .map_err(|error| format!("vkCreateInstance failed: {error:?}"))?;

    let result = (|| {
        // SAFETY: instance is live and output is owned by ash.
        let physical_devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|error| format!("vkEnumeratePhysicalDevices failed: {error:?}"))?;
        if physical_devices.is_empty() {
            return Err("Vulkan loader reported no physical devices".to_owned());
        }

        let mut candidates = Vec::new();
        for physical in physical_devices {
            // SAFETY: physical belongs to the live instance.
            let properties = unsafe { instance.get_physical_device_properties(physical) };
            // SAFETY: Vulkan guarantees a nul-terminated device_name.
            let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: physical belongs to the live instance.
            let families = unsafe {
                instance.get_physical_device_queue_family_properties(physical)
            };
            let family = families.iter().position(|family| {
                family.queue_count > 0
                    && family.queue_flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
            });
            if let Some(family) = family {
                let score = match properties.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                    _ => 0,
                };
                candidates.push((score, physical, family as u32, name, properties.api_version));
            }
        }
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
        let Some((_, physical, family, name, api_version)) = candidates.into_iter().next() else {
            return Err("no Vulkan device has a graphics+compute queue".to_owned());
        };

        let priorities = [1.0_f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_info);

        // SAFETY: selected physical device and queue family are valid.
        let device = unsafe { instance.create_device(physical, &device_info, None) }
            .map_err(|error| format!("vkCreateDevice for {name} failed: {error:?}"))?;
        // SAFETY: queue index zero was requested above.
        let queue = unsafe { device.get_device_queue(family, 0) };
        let version = vk::api_version_major(api_version) * 100
            + vk::api_version_minor(api_version) * 10
            + vk::api_version_patch(api_version);
        tracing::info!(
            device = %name,
            vulkan_api = version,
            queue_family = family,
            queue = ?queue,
            "Vulkan device initialized through Proton"
        );

        // SAFETY: no work was submitted; device is idle and owned here.
        unsafe {
            device.destroy_device(None);
        }
        Ok(())
    })();

    // SAFETY: all child devices have been destroyed before the instance.
    unsafe {
        instance.destroy_instance(None);
    }
    result
}

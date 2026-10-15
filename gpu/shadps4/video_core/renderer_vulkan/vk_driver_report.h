// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: one always-on line in last_run.log with the Vulkan driver and the device paths it
// decides, printed after vk_instance.cpp creates the device. The driver otherwise only reaches
// LOG_INFO, which the default log hides, and AMD keeps the Adrenalin version in driverInfo, not
// in driverVersion. The flags say which code the tester's driver ran (#121): without
// maintenance8 depth/colour overlap copies go through a buffer (vk_runtime.cpp), with EDS3 the
// pipelines pass a null pDepthStencilState (vk_graphics_pipeline.cpp).

#pragma once

#include <cstdint>
#include <cstdio>
#include <string>

namespace Vulkan::DriverReport {

struct Info {
    std::string driver_name;           ///< VkPhysicalDeviceDriverProperties::driverName
    std::string driver_info;           ///< driverInfo, may be empty
    std::string driver_version;        ///< Instance::GetDriverVersionName()
    std::uint32_t api_version{};       ///< VkPhysicalDeviceProperties::apiVersion
    std::string driver_id;             ///< vk::to_string of the driver ID
    std::uint32_t vendor_id{};         ///< PCI vendor ID (or a Khronos vendor ID)
    std::uint32_t device_id{};         ///< PCI device ID
    bool maintenance8{};               ///< VK_KHR_maintenance8 enabled
    bool eds3{};                       ///< VK_EXT_extended_dynamic_state3 enabled
    bool eds3_color_write_mask{};      ///< its extendedDynamicState3ColorWriteMask feature
    bool depth_bounds{};               ///< the depthBounds feature
    bool vertex_input_dynamic_state{}; ///< VK_EXT_vertex_input_dynamic_state enabled
    bool amd_mixed_samples{};          ///< VK_AMD_mixed_attachment_samples enabled
    bool pipeline_binaries{};          ///< the VK_KHR_pipeline_binary cache is in use
};

inline std::string Line(const Info& info) {
    const auto flag = [](bool on) { return on ? '1' : '0'; };
    // apiVersion packs major/minor/patch as 7/10/12 bits under the 3 variant bits.
    const std::string api = std::to_string((info.api_version >> 22) & 0x7f) + '.' +
                            std::to_string((info.api_version >> 12) & 0x3ff) + '.' +
                            std::to_string(info.api_version & 0xfff);
    char pci[24];
    std::snprintf(pci, sizeof(pci), "%04x:%04x", info.vendor_id, info.device_id);
    std::string out = "GPU: driver " + info.driver_name + ", info " +
                      (info.driver_info.empty() ? std::string("-") : info.driver_info) +
                      ", version " + info.driver_version + ", Vulkan " + api + ", id " +
                      info.driver_id + ", pci " + pci;
    out += std::string("; maintenance8 ") + flag(info.maintenance8) + ", eds3 " +
           flag(info.eds3) + " (color write mask " + flag(info.eds3_color_write_mask) +
           "), depth bounds " + flag(info.depth_bounds) + ", vertex input dynamic " +
           flag(info.vertex_input_dynamic_state) + ", amd mixed samples " +
           flag(info.amd_mixed_samples) + ", pipeline binaries " + flag(info.pipeline_binaries);
    return out;
}

} // namespace Vulkan::DriverReport

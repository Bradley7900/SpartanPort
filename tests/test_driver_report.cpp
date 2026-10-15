// SPDX-License-Identifier: GPL-2.0-or-later
// The always-on driver line (vk_driver_report.h): driver name and info, version, Vulkan API,
// driver ID, PCI IDs and the extension/feature paths the device was created with (#121).
// No game or Vulkan device required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <string>

#include "video_core/renderer_vulkan/vk_driver_report.h"

using namespace Vulkan;

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    // An RX 6600 on a 2024 Adrenalin driver: no maintenance8 and no pipeline binaries.
    DriverReport::Info amd{};
    amd.driver_name = "AMD proprietary driver";
    amd.driver_info = "24.5.1 (AMD proprietary shader compiler)";
    amd.driver_version = "2.0.302";
    amd.api_version = (1u << 22) | (3u << 12) | 280u;
    amd.driver_id = "AmdProprietary";
    amd.vendor_id = 0x1002;
    amd.device_id = 0x73ff;
    amd.maintenance8 = false;
    amd.eds3 = true;
    amd.eds3_color_write_mask = true;
    amd.depth_bounds = true;
    amd.vertex_input_dynamic_state = true;
    amd.amd_mixed_samples = true;
    amd.pipeline_binaries = false;
    assert(DriverReport::Line(amd) ==
           "GPU: driver AMD proprietary driver, info 24.5.1 (AMD proprietary shader compiler),"
           " version 2.0.302, Vulkan 1.3.280, id AmdProprietary, pci 1002:73ff;"
           " maintenance8 0, eds3 1 (color write mask 1), depth bounds 1,"
           " vertex input dynamic 1, amd mixed samples 1, pipeline binaries 0");

    // An empty driverInfo prints as '-', the API patch can exceed 255, the PCI IDs are
    // zero-padded and every flag can be on.
    DriverReport::Info intel{};
    intel.driver_name = "Intel Corporation";
    intel.driver_version = "101.6129";
    intel.api_version = (1u << 22) | (4u << 12) | 309u;
    intel.driver_id = "IntelProprietaryWindows";
    intel.vendor_id = 0x8086;
    intel.device_id = 0x56a0;
    intel.maintenance8 = true;
    intel.eds3 = true;
    intel.eds3_color_write_mask = true;
    intel.depth_bounds = true;
    intel.vertex_input_dynamic_state = true;
    intel.amd_mixed_samples = true;
    intel.pipeline_binaries = true;
    assert(DriverReport::Line(intel) ==
           "GPU: driver Intel Corporation, info -, version 101.6129, Vulkan 1.4.309,"
           " id IntelProprietaryWindows, pci 8086:56a0; maintenance8 1,"
           " eds3 1 (color write mask 1), depth bounds 1, vertex input dynamic 1,"
           " amd mixed samples 1, pipeline binaries 1");

    // Nothing optional: small IDs keep four hex digits and Khronos vendor IDs (above 0xffff)
    // print in full.
    DriverReport::Info bare{};
    bare.driver_name = "llvmpipe";
    bare.driver_info = "Mesa 24.0.5";
    bare.driver_version = "0.0.1";
    bare.api_version = (1u << 22) | (3u << 12);
    bare.driver_id = "MesaLlvmpipe";
    bare.vendor_id = 0x10005;
    bare.device_id = 0x0;
    assert(DriverReport::Line(bare) ==
           "GPU: driver llvmpipe, info Mesa 24.0.5, version 0.0.1, Vulkan 1.3.0,"
           " id MesaLlvmpipe, pci 10005:0000; maintenance8 0, eds3 0 (color write mask 0),"
           " depth bounds 0, vertex input dynamic 0, amd mixed samples 0, pipeline binaries 0");

    std::printf("driver report: ok\n");
    return 0;
}

// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: the "Compiling graphics pipeline" line the pipeline cache logs right before the driver
// compiles a pipeline: on the GPU thread and on the compile workers (vk_pipeline_cache.cpp), and
// when the preload or the background warm-up rebuilds one from the disk cache
// (vk_pipeline_serialization.cpp). A crash inside the driver's shader compiler (igc64.dll on
// Intel, #35) then leaves the pipeline and the hashes of its shaders as the last lines of
// last_run.log. Also the level of the per-shader "Compiling <stage> shader" lines. Pure std/fmt,
// so tests/test_compile_log.cpp checks the text without a Vulkan device.

#pragma once

#include <cstdint>
#include <span>
#include <string>
#include <string_view>
#include <fmt/format.h>

namespace Vulkan {

struct CompileLogStage {
    std::string name;     ///< Hardware stage as the shader logs print it: "vs", "fs", ...
    std::uint64_t hash{}; ///< Shader::Info::pgm_hash
};

/// "0x<pipeline> (vs 0x<hash>, fs 0x<hash>)"; a non-empty `mode` ("async" on a compile worker,
/// "cache" for the preload and warm-up) comes first in the parenthesis, followed by "; ".
inline std::string FormatPipelineCompile(std::uint64_t pipeline_hash,
                                         std::span<const CompileLogStage> stages,
                                         std::string_view mode) {
    std::string details{mode};
    const char* separator = mode.empty() ? "" : "; ";
    for (const auto& stage : stages) {
        details += fmt::format("{}{} {:#x}", separator, stage.name, stage.hash);
        separator = ", ";
    }
    return details.empty() ? fmt::format("{:#x}", pipeline_hash)
                           : fmt::format("{:#x} ({})", pipeline_hash, details);
}

/// Whether the "Compiling <stage> shader 0x<hash> (<mode>; permutation N)" line logged before
/// vkCreateShaderModule goes to Info. The driver runs its shader compiler when the pipeline is
/// created (that line stays at Info), so the one line per shader the preload and the warm-up
/// rebuild from the disk cache ("cache") is Debug: on a warm start those lines were over a third
/// of last_run.log (#35). The GPU thread's ("") and the compile workers' ("async") stay at Info.
inline bool LogsShaderModuleCompileAtInfo(std::string_view mode) {
    return mode != "cache";
}

} // namespace Vulkan

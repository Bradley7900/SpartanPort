// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: the "Compiling graphics pipeline" line the pipeline cache logs right before the driver
// compiles a pipeline (vk_pipeline_cache.cpp), on the GPU thread and on the compile workers. A
// crash inside the driver's shader compiler (igc64.dll on Intel, #35) then leaves the pipeline
// and the hashes of its shaders as the last lines of last_run.log. Pure std/fmt, so
// tests/test_compile_log.cpp checks the text without a Vulkan device.

#pragma once

#include <cstdint>
#include <span>
#include <string>
#include <fmt/format.h>

namespace Vulkan {

struct CompileLogStage {
    std::string name;     ///< Hardware stage as the shader logs print it: "vs", "fs", ...
    std::uint64_t hash{}; ///< Shader::Info::pgm_hash
};

/// "0x<pipeline> (vs 0x<hash>, fs 0x<hash>)"; a compile worker adds "async; " after the
/// parenthesis.
inline std::string FormatPipelineCompile(std::uint64_t pipeline_hash,
                                         std::span<const CompileLogStage> stages, bool async) {
    std::string details = async ? "async" : "";
    const char* separator = async ? "; " : "";
    for (const auto& stage : stages) {
        details += fmt::format("{}{} {:#x}", separator, stage.name, stage.hash);
        separator = ", ";
    }
    return details.empty() ? fmt::format("{:#x}", pipeline_hash)
                           : fmt::format("{:#x} ({})", pipeline_hash, details);
}

} // namespace Vulkan

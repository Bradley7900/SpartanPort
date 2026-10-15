// SPDX-License-Identifier: GPL-2.0-or-later
// The "Compiling graphics pipeline" line (vk_compile_log.h): a crash inside the driver's shader
// compiler (igc64.dll on Intel, #35) must leave the pipeline and its shader hashes as the last
// lines of last_run.log, also when a compile worker builds it or the preload / background
// warm-up rebuilds it from the disk cache. No game, Vulkan device or window required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <span>
#include <vector>

#include "video_core/renderer_vulkan/vk_compile_log.h"

using namespace Vulkan;

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    const std::vector<CompileLogStage> stages{{"vs", 0x1234}, {"fs", 0xabcdef0123456789}};
    // Synchronous compile: the pipeline hash, then every stage in the order given.
    assert(FormatPipelineCompile(0x9f, stages, "") == "0x9f (vs 0x1234, fs 0xabcdef0123456789)");
    // A compile worker marks the line, so the log tells the worker from the GPU thread.
    assert(FormatPipelineCompile(0x9f, stages, "async") ==
           "0x9f (async; vs 0x1234, fs 0xabcdef0123456789)");
    assert(FormatPipelineCompile(0x9f, std::span{stages}.first(1), "async") ==
           "0x9f (async; vs 0x1234)");
    // A pipeline rebuilt from the disk cache (preload, background warm-up) says so.
    assert(FormatPipelineCompile(0x9f, stages, "cache") ==
           "0x9f (cache; vs 0x1234, fs 0xabcdef0123456789)");
    // No stages: only the pipeline hash and the marker.
    assert(FormatPipelineCompile(0x9f, {}, "") == "0x9f");
    assert(FormatPipelineCompile(0x9f, {}, "async") == "0x9f (async)");
    assert(FormatPipelineCompile(0x9f, {}, "cache") == "0x9f (cache)");
    std::puts("compile-log-test: ok");
    return EXIT_SUCCESS;
}

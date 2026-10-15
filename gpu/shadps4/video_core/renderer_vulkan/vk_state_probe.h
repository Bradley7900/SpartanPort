// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: GOW3_STATE_PROBE=viewport,depthstruct,depthreemit (or `all`). Issue #121 test build: the
// opaque scene only reaches a 512x256 box on two RDNA2 machines with 2024 AMD drivers. Each
// token changes one piece of state the port sets the same way on every driver, so a tester can
// tell which one (if any) that driver mishandles:
//   viewport     viewport and scissor recorded again inside the draw, right after each
//                bindPipeline (vk_rasterizer.cpp), not only before vkCmdBeginRendering
//   depthstruct  pDepthStencilState passed (all zeros, everything is dynamic) even with
//                VK_EXT_extended_dynamic_state3, where it is null (vk_graphics_pipeline.cpp)
//   depthreemit  every depth and stencil dynamic state recorded again right after each
//                bindPipeline (vk_rasterizer.cpp, DynamicState::EmitDepthStencil)

#pragma once

#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <string_view>

namespace Vulkan::StateProbe {

struct Flags {
    bool viewport{};
    bool depthstruct{};
    bool depthreemit{};

    bool Any() const {
        return viewport || depthstruct || depthreemit;
    }
    bool operator==(const Flags&) const = default;
};

/// Tokens separated by commas or spaces, case-insensitive; `all` or `1` turn every token on,
/// unknown tokens are ignored.
inline Flags Parse(const char* text) {
    Flags flags;
    if (!text) {
        return flags;
    }
    std::string_view rest{text};
    while (!rest.empty()) {
        const auto end = rest.find_first_of(", ");
        std::string token{rest.substr(0, end)};
        rest = end == std::string_view::npos ? std::string_view{} : rest.substr(end + 1);
        for (auto& c : token) {
            c = char(std::tolower(static_cast<unsigned char>(c)));
        }
        if (token == "all" || token == "1") {
            flags = Flags{.viewport = true, .depthstruct = true, .depthreemit = true};
        } else if (token == "viewport") {
            flags.viewport = true;
        } else if (token == "depthstruct") {
            flags.depthstruct = true;
        } else if (token == "depthreemit") {
            flags.depthreemit = true;
        }
    }
    return flags;
}

/// The line a tester's last_run.log must contain; empty when nothing is on.
inline std::string Marker(Flags flags) {
    if (!flags.Any()) {
        return {};
    }
    std::string line = "GPU: state probe on:";
    if (flags.viewport) {
        line += " viewport";
    }
    if (flags.depthstruct) {
        line += " depthstruct";
    }
    if (flags.depthreemit) {
        line += " depthreemit";
    }
    return line;
}

/// GOW3_STATE_PROBE, read once; the marker goes to stdout (last_run.log) the first time.
inline Flags Current() {
    static const Flags flags = [] {
        const Flags parsed = Parse(std::getenv("GOW3_STATE_PROBE"));
        if (parsed.Any()) {
            std::printf("%s\n", Marker(parsed).c_str());
            std::fflush(stdout);
        }
        return parsed;
    }();
    return flags;
}

} // namespace Vulkan::StateProbe

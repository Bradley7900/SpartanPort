// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: which path Runtime::CopyColorAndDepth (vk_runtime.cpp) takes for a depth/colour overlap
// copy, and its always-on log line. ResolveDepthOverlap (texture_cache.cpp) recreates an image
// as depth or colour over the same guest memory and keeps the old contents only through this
// copy, without clearing the new image; IsComputeImageCopy (vk_rasterizer.cpp) replaces a
// compute copy with it. A copy that covers less than the destination, or copies nothing,
// leaves the rest undefined. The path depends on the driver (maintenance8), so the line tells
// which code the tester's driver ran (#121).

#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <string>
#include <string_view>

namespace Vulkan::OverlapCopy {

enum class Path : std::uint8_t {
    Copy,           ///< vkCmdCopyImage at the source's size
    ViaBuffer,      ///< depth <-> colour through a staging buffer (no VK_KHR_maintenance8)
    ColorToMsDepth, ///< a draw writing the source's channels as depth samples (blit_helper)
    Unimplemented,  ///< nothing is copied
};

inline const char* Name(Path path) {
    switch (path) {
    case Path::Copy:
        return "copy";
    case Path::ViaBuffer:
        return "via-buffer";
    case Path::ColorToMsDepth:
        return "color-to-ms-depth";
    case Path::Unimplemented:
        return "unimplemented";
    }
    return "?";
}

/// The branches of CopyColorAndDepth: host sample counts, whether each image is depth, the
/// destination's guest sample count and whether VK_KHR_maintenance8 is enabled. With
/// GOW3_NO_MSAA a guest multisampled depth target is single-sampled on the host but the colour
/// alias still holds the samples as channels, so it takes the multisample path.
constexpr Path Choose(std::uint32_t src_samples, std::uint32_t dst_samples, bool src_depth,
                      bool dst_depth, std::uint32_t dst_guest_samples, bool maintenance8) {
    const bool guest_ms_depth = dst_depth && dst_guest_samples > 1;
    if (src_samples == 1 && dst_samples == 1 && !guest_ms_depth) {
        return maintenance8 || src_depth == dst_depth ? Path::Copy : Path::ViaBuffer;
    }
    if (src_samples == 1 && (dst_samples > 1 || guest_ms_depth) && dst_depth) {
        return Path::ColorToMsDepth;
    }
    return Path::Unimplemented;
}

/// One image of the copy; `format` is already a name (vk::to_string of the host format).
struct Side {
    std::uint64_t address{}; ///< guest address
    std::string format;
    std::uint32_t width{};
    std::uint32_t height{};
    std::uint32_t levels{};
    std::uint32_t layers{};
    std::uint32_t samples{};       ///< host sample count
    std::uint32_t guest_samples{}; ///< guest sample count
    bool depth{};
};

struct LogBudget {
    static constexpr std::uint32_t Cap = 32;

    /// Counts a copy down `path`. Returns its line number (1..Cap) while lines remain, Cap + 1
    /// for the first copy past them (print LimitLine() instead), then 0 without counting.
    std::uint32_t Take(Path path) {
        if (taken > Cap) {
            return 0;
        }
        ++counts[static_cast<std::size_t>(path)];
        return ++taken;
    }

    std::string LimitLine() const {
        return "GPU: overlap copy: log limit reached (copy " + std::to_string(counts[0]) +
               ", via-buffer " + std::to_string(counts[1]) + ", color-to-ms-depth " +
               std::to_string(counts[2]) + ", unimplemented " + std::to_string(counts[3]) + ")";
    }

    std::uint32_t taken{};
    std::array<std::uint32_t, 4> counts{};
};

inline std::string Describe(const Side& side) {
    char head[32];
    std::snprintf(head, sizeof(head), "0x%llx ", static_cast<unsigned long long>(side.address));
    return head + side.format + ' ' + std::to_string(side.width) + 'x' +
           std::to_string(side.height) + " M" + std::to_string(side.levels) + " L" +
           std::to_string(side.layers) + " S" + std::to_string(side.samples) + '/' +
           std::to_string(side.guest_samples) + (side.depth ? " depth" : " color");
}

/// `number` is the line's place in the budget (1..LogBudget::Cap); 0 leaves the counter out
/// (a frame capture note past the budget). `reason` names the caller.
inline std::string Line(std::uint32_t number, Path path, std::string_view reason, const Side& src,
                        const Side& dst) {
    std::string out = "GPU: overlap copy ";
    if (number != 0) {
        out += std::to_string(number) + '/' + std::to_string(LogBudget::Cap) + ' ';
    }
    out += std::string(Name(path)) + " (" + std::string(reason) + "): " + Describe(src) + " -> " +
           Describe(dst);
    if (src.width != dst.width || src.height != dst.height) {
        out += " size-mismatch";
    }
    return out;
}

} // namespace Vulkan::OverlapCopy

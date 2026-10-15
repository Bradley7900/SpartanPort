// SPDX-License-Identifier: GPL-2.0-or-later
// The path a depth/colour overlap copy takes (vk_overlap_copy.h, Runtime::CopyColorAndDepth in
// vk_runtime.cpp), its always-on log line and the 32-line log budget (#121). No game or Vulkan
// device required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <string>

#include "video_core/renderer_vulkan/vk_overlap_copy.h"

using namespace Vulkan;

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    using OverlapCopy::Choose;
    using OverlapCopy::Path;
    // Arguments: src samples, dst samples, src depth, dst depth, dst guest samples, maintenance8.

    // Single-sampled on both sides and the same kind: a plain image copy, with or without
    // maintenance8.
    assert(Choose(1, 1, false, false, 1, false) == Path::Copy);
    assert(Choose(1, 1, true, true, 1, false) == Path::Copy);
    assert(Choose(1, 1, true, true, 1, true) == Path::Copy);
    // Colour <-> depth: an image copy needs maintenance8, otherwise it goes through a buffer.
    assert(Choose(1, 1, false, true, 1, false) == Path::ViaBuffer);
    assert(Choose(1, 1, true, false, 1, false) == Path::ViaBuffer);
    assert(Choose(1, 1, false, true, 1, true) == Path::Copy);
    assert(Choose(1, 1, true, false, 1, true) == Path::Copy);
    // The guest sample count only matters for a depth destination: a colour destination with
    // guest MSAA is still copied.
    assert(Choose(1, 1, true, false, 4, false) == Path::ViaBuffer);
    assert(Choose(1, 1, false, false, 2, true) == Path::Copy);

    // GOW3_NO_MSAA: a guest 2x depth target is 1x on the host, but the colour alias still holds
    // the samples as channels, so it is drawn sample by sample, not copied.
    assert(Choose(1, 1, false, true, 2, false) == Path::ColorToMsDepth);
    assert(Choose(1, 1, false, true, 2, true) == Path::ColorToMsDepth);
    // A multisampled host depth target, from colour or (the code does not check) from depth.
    assert(Choose(1, 4, false, true, 4, false) == Path::ColorToMsDepth);
    assert(Choose(1, 2, true, true, 2, true) == Path::ColorToMsDepth);

    // Everything else copies nothing: a multisampled source, or a multisampled colour
    // destination.
    assert(Choose(4, 4, false, true, 4, true) == Path::Unimplemented);
    assert(Choose(2, 1, false, false, 1, true) == Path::Unimplemented);
    assert(Choose(2, 1, true, true, 2, false) == Path::Unimplemented);
    assert(Choose(1, 4, false, false, 4, true) == Path::Unimplemented);
    assert(Choose(1, 4, true, false, 4, false) == Path::Unimplemented);
    static_assert(Choose(1, 1, false, true, 1, false) == Path::ViaBuffer);

    assert(std::string(OverlapCopy::Name(Path::Copy)) == "copy");
    assert(std::string(OverlapCopy::Name(Path::ViaBuffer)) == "via-buffer");
    assert(std::string(OverlapCopy::Name(Path::ColorToMsDepth)) == "color-to-ms-depth");
    assert(std::string(OverlapCopy::Name(Path::Unimplemented)) == "unimplemented");

    // The log line: number out of the budget, path, caller, then each image's guest address,
    // host format, size, mips, layers, host/guest samples and kind. Different sizes are flagged:
    // the copy covers only the source's size (or stretches it, for color-to-ms-depth).
    OverlapCopy::Side color{};
    color.address = 0x7f12340000;
    color.format = "R32Sfloat";
    color.width = 512;
    color.height = 256;
    color.levels = 1;
    color.layers = 1;
    color.samples = 1;
    color.guest_samples = 2;
    color.depth = false;
    OverlapCopy::Side depth{};
    depth.address = 0x7f12340000;
    depth.format = "D32SfloatS8Uint";
    depth.width = 1280;
    depth.height = 720;
    depth.levels = 1;
    depth.layers = 1;
    depth.samples = 1;
    depth.guest_samples = 2;
    depth.depth = true;
    assert(OverlapCopy::Line(3, Path::ColorToMsDepth, "depth-overlap", color, depth) ==
           "GPU: overlap copy 3/32 color-to-ms-depth (depth-overlap): 0x7f12340000 R32Sfloat"
           " 512x256 M1 L1 S1/2 color -> 0x7f12340000 D32SfloatS8Uint 1280x720 M1 L1 S1/2"
           " depth size-mismatch");

    // Same size on both sides: no flag. Height alone differing is a mismatch too.
    OverlapCopy::Side dst = color;
    dst.address = 0x400000;
    dst.format = "R32Uint";
    dst.levels = 3;
    dst.layers = 6;
    dst.samples = 4;
    dst.guest_samples = 4;
    assert(OverlapCopy::Line(32, Path::Unimplemented, "compute-copy", color, dst) ==
           "GPU: overlap copy 32/32 unimplemented (compute-copy): 0x7f12340000 R32Sfloat"
           " 512x256 M1 L1 S1/2 color -> 0x400000 R32Uint 512x256 M3 L6 S4/4 color");
    dst.height = 128;
    assert(OverlapCopy::Line(1, Path::Copy, "compute-copy", color, dst) ==
           "GPU: overlap copy 1/32 copy (compute-copy): 0x7f12340000 R32Sfloat 512x256 M1 L1"
           " S1/2 color -> 0x400000 R32Uint 512x128 M3 L6 S4/4 color size-mismatch");
    // Number 0: a capture note past the budget, without the counter.
    assert(OverlapCopy::Line(0, Path::Copy, "compute-copy", color, color) ==
           "GPU: overlap copy copy (compute-copy): 0x7f12340000 R32Sfloat 512x256 M1 L1 S1/2"
           " color -> 0x7f12340000 R32Sfloat 512x256 M1 L1 S1/2 color");

    // The budget: lines 1..32, then the copy that prints the limit line, then silence. The
    // limit line counts every copy up to and including that one, per path.
    OverlapCopy::LogBudget budget{};
    static_assert(OverlapCopy::LogBudget::Cap == 32);
    for (unsigned i = 1; i <= 20; ++i) {
        assert(budget.Take(Path::Copy) == i);
    }
    for (unsigned i = 21; i <= 30; ++i) {
        assert(budget.Take(Path::ViaBuffer) == i);
    }
    assert(budget.Take(Path::ColorToMsDepth) == 31);
    assert(budget.Take(Path::ColorToMsDepth) == 32);
    assert(budget.Take(Path::Unimplemented) == 33);
    assert(budget.LimitLine() ==
           "GPU: overlap copy: log limit reached (copy 20, via-buffer 10, color-to-ms-depth 2,"
           " unimplemented 1)");
    for (unsigned i = 0; i < 100; ++i) {
        assert(budget.Take(Path::Copy) == 0);
    }
    // The limit line keeps the counts it had when the budget ran out.
    assert(budget.LimitLine() ==
           "GPU: overlap copy: log limit reached (copy 20, via-buffer 10, color-to-ms-depth 2,"
           " unimplemented 1)");

    // A fresh budget starts at line 1 (one budget per Runtime).
    OverlapCopy::LogBudget other{};
    assert(other.Take(Path::Unimplemented) == 1);

    std::printf("overlap copy: ok\n");
    return 0;
}

// SPDX-License-Identifier: GPL-2.0-or-later
// The render-area check of a pass (vk_area_check.h, Rasterizer::CheckRenderArea): the area
// against the extent each attachment's view really has, the verdicts, the log lines and their
// budget, and the one frame capture per run (#121). No game or Vulkan device required.
#include <array>
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <span>
#include <string>

#include "video_core/renderer_vulkan/vk_area_check.h"

using namespace Vulkan;
using AreaCheck::Attachment;
using AreaCheck::Extent;
using AreaCheck::Override;
using AreaCheck::Pass;
using AreaCheck::Reporter;
using AreaCheck::Result;
using AreaCheck::Verdict;

namespace {

Attachment Color(std::uint8_t slot, std::uint64_t address, std::uint32_t format, Extent size) {
    return {.address = address,
            .format = format,
            .native = size,
            .bound = size,
            .slot = slot,
            .mip = 0,
            .samples = 1,
            .guest_samples = 1,
            .depth = false};
}

Attachment Depth(std::uint64_t address, std::uint32_t format, Extent size) {
    Attachment depth = Color(0, address, format, size);
    depth.depth = true;
    return depth;
}

/// Five G-buffer colour targets of `color` and a depth target of `depth`.
std::array<Attachment, 6> GBuffer(Extent color, Extent depth) {
    return {Color(0, 0x10000000, 37, color), Color(1, 0x11000000, 37, color),
            Color(2, 0x12000000, 37, color), Color(3, 0x13000000, 37, color),
            Color(4, 0x14000000, 97, color), Depth(0x18000000, 130, depth)};
}

Result CheckArea(Extent area, std::span<const Attachment> attachments, bool scene = true,
             Extent viewport = {}) {
    Pass pass{};
    pass.area = area;
    pass.viewport = viewport;
    pass.scene = scene;
    return AreaCheck::Check(pass, attachments);
}

std::string FormatName(std::uint32_t format) {
    return "F" + std::to_string(format);
}

} // namespace

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    constexpr std::uint32_t kD = 1u << AreaCheck::kDepthBit;

    // 1. A G-buffer pass at 1920x1080: the depth target is padded to 1088 lines (PS4 tiling),
    // which is legitimate.
    {
        const auto gbuffer = GBuffer({1920, 1080}, {1920, 1088});
        const Result r = CheckArea({1920, 1080}, gbuffer);
        assert(r.verdict == Verdict::Ok);
        assert(r.padded_mask == kD);
        assert(r.larger_mask == 0 && r.smaller_mask == 0);
        assert(!r.high);
        assert(AreaCheck::Short(r, Pass{}) == "ok");
    }

    // 2. The #121 shape: one G-buffer target is 512x256 and the area follows it (the minimum of
    // the attachments). It names the limiter; severe only on a scene pass.
    auto issue121 = GBuffer({1920, 1080}, {1920, 1088});
    issue121[4].native = issue121[4].bound = {512, 256};
    {
        const Result r = CheckArea({512, 256}, issue121);
        assert(r.verdict == Verdict::MixedSizes);
        assert(r.larger_mask == (0x0fu | kD));
        assert(r.limiter_mask == 0x10u);
        assert(r.high);
        assert(!CheckArea({512, 256}, issue121, false).high);
        assert(AreaCheck::Short(r, Pass{}) == "mixed-sizes larger c0,c1,c2,c3,d limiter c4");
    }

    // 3. Every attachment is 1920x1080 but the area is 512x256: nothing explains it.
    {
        const auto gbuffer = GBuffer({1920, 1080}, {1920, 1080});
        const Result r = CheckArea({512, 256}, gbuffer, false);
        assert(r.verdict == Verdict::AreaBelowAll);
        assert(r.larger_mask == (0x1fu | kD));
        assert(r.limiter_mask == 0);
        assert(r.high);
        assert(AreaCheck::Short(r, Pass{}) == "area-below-all larger c0,c1,c2,c3,c4,d");
    }

    // 3b. Only one dimension is unexplained: every attachment is 1920x1080 and the area is
    // 1920x256. The width matches all of them, the height none.
    {
        const auto gbuffer = GBuffer({1920, 1080}, {1920, 1080});
        const Result r = CheckArea({1920, 256}, gbuffer, false);
        assert(r.verdict == Verdict::AreaBelowAll);
        assert(r.high);
    }

    // 4. A reduced pass: the views are the 1280x720 scene proxies and so is the area. Compared
    // with the native images it would be flagged, which is why bound extents are used.
    {
        Override reduced{};
        reduced.proxy = {1280, 720};
        auto gbuffer = GBuffer({1920, 1080}, {1920, 1080});
        for (auto& a : gbuffer) {
            a.bound = AreaCheck::Bound(reduced, a.native, a.depth, a.slot);
            assert((a.bound == Extent{1280, 720}));
        }
        Pass pass{};
        pass.area = {1280, 720};
        pass.overrides = reduced;
        pass.scene = true;
        const Result r = AreaCheck::Check(pass, gbuffer);
        assert(r.verdict == Verdict::Ok);
        assert(AreaCheck::Short(r, pass) == "ok reduced");
        for (auto& a : gbuffer) {
            a.bound = a.native;
        }
        assert(AreaCheck::Check(pass, gbuffer).verdict == Verdict::AreaBelowAll);
    }

    // 5. An accepted upscaler redirect: colour 0 is drawn into the 3840x2160 output image and
    // the area is its size.
    {
        Override redirected{};
        redirected.color_redirect = {3840, 2160};
        assert((AreaCheck::Bound(redirected, {1920, 1080}, false, 0) == Extent{3840, 2160}));
        assert((AreaCheck::Bound(redirected, {1920, 1080}, false, 1) == Extent{1920, 1080}));
        std::array<Attachment, 1> ui = {Color(0, 0x20000000, 50, {1920, 1080})};
        ui[0].bound = AreaCheck::Bound(redirected, ui[0].native, false, 0);
        Pass pass{};
        pass.area = {3840, 2160};
        pass.overrides = redirected;
        const Result r = AreaCheck::Check(pass, ui);
        assert(r.verdict == Verdict::Ok);
        assert(AreaCheck::Short(r, pass) == "ok redirected");
    }

    // 6. A redirect rejected on a reduced pass: colour 0 goes back to its proxy view, but the
    // depth target goes back to its native view while the area stays the proxy size.
    Override rejected{};
    rejected.proxy = {1280, 720};
    rejected.depth_redirect = {3840, 2160};
    rejected.rejected = true;
    auto rejected_pass = GBuffer({1920, 1080}, {1920, 1080});
    for (auto& a : rejected_pass) {
        a.bound = AreaCheck::Bound(rejected, a.native, a.depth, a.slot);
    }
    assert((rejected_pass[0].bound == Extent{1280, 720}));
    assert((rejected_pass[5].bound == Extent{1920, 1080}));
    {
        Pass pass{};
        pass.area = {1280, 720};
        pass.overrides = rejected;
        pass.scene = true;
        const Result r = AreaCheck::Check(pass, rejected_pass);
        assert(r.verdict == Verdict::MixedSizes);
        assert(r.larger_mask == kD && r.limiter_mask == 0x1fu);
        assert(r.high);
        assert(AreaCheck::Short(r, pass) ==
               "mixed-sizes reduced+redirect-rejected larger d limiter c0,c1,c2,c3,c4");
    }
    {
        // Colour 0 redirected and the redirect rejected: back to the proxy (reduced) or native.
        Override color_rejected{};
        color_rejected.color_redirect = {3840, 2160};
        color_rejected.rejected = true;
        assert((AreaCheck::Bound(color_rejected, {1920, 1080}, false, 0) == Extent{1920, 1080}));
        color_rejected.proxy = {1280, 720};
        assert((AreaCheck::Bound(color_rejected, {1920, 1080}, false, 0) == Extent{1280, 720}));
        // An accepted depth redirect.
        Override depth_redirected{};
        depth_redirected.depth_redirect = {3840, 2160};
        assert((AreaCheck::Bound(depth_redirected, {1920, 1080}, true, 0) == Extent{3840, 2160}));
        assert((AreaCheck::Bound(depth_redirected, {1920, 1080}, false, 0) == Extent{1920, 1080}));
    }
    assert(AreaCheck::Name(Override{}) == "none");
    assert(AreaCheck::Name(rejected) == "reduced+redirect-rejected");

    // 7. Every attachment wrongly 512x256: the area matches them, only the guest viewport shows
    // that the game draws 1920x1080.
    {
        const auto small = GBuffer({512, 256}, {512, 256});
        const Result r = CheckArea({512, 256}, small, false, {1920, 1080});
        assert(r.verdict == Verdict::ViewportBeyondArea);
        assert(r.high);
        Pass pass{};
        pass.area = {512, 256};
        pass.viewport = {1920, 1080};
        assert(AreaCheck::Short(r, pass) == "viewport-beyond-area viewport 1920x1080");
        constexpr auto kBeyond = Verdict::ViewportBeyondArea;
        assert(CheckArea({512, 256}, small, false, {575, 319}).verdict == Verdict::Ok);
        assert(CheckArea({512, 256}, small, false, {576, 256}).verdict == kBeyond);
        assert(CheckArea({512, 256}, small, false, {512, 320}).verdict == kBeyond);
        // Unknown viewport (clip disabled or no viewport).
        assert(CheckArea({512, 256}, small, false, {}).verdict == Verdict::Ok);
    }

    // 8. 63 pixels of slack are padding, 64 are an area smaller than the attachment.
    {
        const auto tall_ok = GBuffer({1920, 1080}, {1920, 1143});
        const auto wide_ok = GBuffer({1920, 1080}, {1983, 1080});
        assert(CheckArea({1920, 1080}, tall_ok).verdict == Verdict::Ok);
        assert(CheckArea({1920, 1080}, wide_ok).verdict == Verdict::Ok);
        const Result tall = CheckArea({1920, 1080}, GBuffer({1920, 1080}, {1920, 1144}));
        assert(tall.verdict == Verdict::MixedSizes && tall.larger_mask == kD);
        const Result wide = CheckArea({1920, 1080}, GBuffer({1920, 1080}, {1984, 1080}));
        assert(wide.verdict == Verdict::MixedSizes && wide.larger_mask == kD);
        // Each dimension is explained by some attachment: still only mixed sizes.
        std::array<Attachment, 2> split = {Color(0, 0x1000, 37, {512, 1080}),
                                           Color(1, 0x2000, 37, {1920, 256})};
        const Result r = CheckArea({512, 256}, split, false);
        assert(r.verdict == Verdict::MixedSizes);
        assert(r.larger_mask == 0x3u && r.limiter_mask == 0x3u);
        assert(!r.high);
    }

    // 9. An area larger than an attachment's view (not allowed by Vulkan).
    {
        auto gbuffer = GBuffer({1920, 1080}, {1920, 1080});
        gbuffer[1].bound = {1280, 720};
        const Result r = CheckArea({1920, 1080}, gbuffer, false);
        assert(r.verdict == Verdict::AreaExceeds);
        assert(r.smaller_mask == 0x2u);
        assert(r.high);
        assert(AreaCheck::Short(r, Pass{}) == "area-exceeds smaller c1");
    }

    // 10. No attachments: nothing to compare.
    {
        const Result r = CheckArea({16384, 16384}, {});
        assert(r.verdict == Verdict::Ok && !r.high);
        assert(r.larger_mask == 0 && r.limiter_mask == 0 && r.padded_mask == 0 &&
               r.smaller_mask == 0);
    }

    // 12. The log lines, one per attachment after the pass line.
    {
        Pass pass{};
        pass.area = {512, 256};
        pass.scene = true;
        pass.vs_hash = 0x1234abcd;
        pass.ps_hash = 0x89abcdef;
        pass.mrt_mask = 0x1f;
        const Result r = AreaCheck::Check(pass, issue121);
        assert(AreaCheck::Format(r, pass, issue121, FormatName) ==
               "GPU: area check: mixed-sizes area 512x256 override none viewport - scene 1"
               " vs 1234abcd ps 89abcdef mrt 0x1f scale 1.000x1.000\n"
               "GPU: area check:   c0 0x10000000 F37 native 1920x1080 bound 1920x1080 mip 0"
               " samples 1/1 larger\n"
               "GPU: area check:   c1 0x11000000 F37 native 1920x1080 bound 1920x1080 mip 0"
               " samples 1/1 larger\n"
               "GPU: area check:   c2 0x12000000 F37 native 1920x1080 bound 1920x1080 mip 0"
               " samples 1/1 larger\n"
               "GPU: area check:   c3 0x13000000 F37 native 1920x1080 bound 1920x1080 mip 0"
               " samples 1/1 larger\n"
               "GPU: area check:   c4 0x14000000 F97 native 512x256 bound 512x256 mip 0"
               " samples 1/1 limiter\n"
               "GPU: area check:   d 0x18000000 F130 native 1920x1088 bound 1920x1088 mip 0"
               " samples 1/1 larger");

        // An override, a known viewport, a scale, a mip level, guest MSAA and padding.
        std::array<Attachment, 2> mixed = {Color(2, 0x30000000, 44, {960, 540}),
                                           Depth(0x31000000, 126, {1920, 1080})};
        mixed[0].mip = 1;
        mixed[0].guest_samples = 4;
        mixed[0].bound = {1280, 736};
        mixed[1].bound = mixed[1].native;
        Pass scaled{};
        scaled.area = {1280, 720};
        scaled.overrides.proxy = {1280, 720};
        scaled.viewport = {1280, 720};
        scaled.vs_hash = 0xfedcba9876543210ull;
        scaled.mrt_mask = 0x4;
        scaled.scale_x = 1280.0f / 1920.0f;
        scaled.scale_y = 720.0f / 1080.0f;
        const Result m = AreaCheck::Check(scaled, mixed);
        assert(m.verdict == Verdict::MixedSizes && !m.high);
        assert(m.padded_mask == 0x4u);
        assert(AreaCheck::Format(m, scaled, mixed, FormatName) ==
               "GPU: area check: mixed-sizes area 1280x720 override reduced viewport 1280x720"
               " scene 0 vs fedcba9876543210 ps 00000000 mrt 0x4 scale 0.667x0.667\n"
               "GPU: area check:   c2 0x30000000 F44 native 960x540 bound 1280x736 mip 1"
               " samples 1/4 padded limiter\n"
               "GPU: area check:   d 0x31000000 F126 native 1920x1080 bound 1920x1080 mip 0"
               " samples 1/1 larger");
    }

    // The dedupe signature: the attachments, the area, the override and the verdict, not the
    // shaders that drew the pass.
    {
        Pass pass{};
        pass.area = {512, 256};
        pass.scene = true;
        const Result r = AreaCheck::Check(pass, issue121);
        const auto sig = AreaCheck::Signature(r, pass, issue121);
        Pass other_shaders = pass;
        other_shaders.vs_hash = 1;
        other_shaders.ps_hash = 2;
        assert(AreaCheck::Signature(r, other_shaders, issue121) == sig);
        auto moved = issue121;
        moved[4].address += 0x1000;
        assert(AreaCheck::Signature(r, pass, moved) != sig);
        auto rebound = issue121;
        rebound[0].bound = {1920, 1088};
        assert(AreaCheck::Signature(r, pass, rebound) != sig);
        Pass reduced = pass;
        reduced.overrides.proxy = {512, 256};
        assert(AreaCheck::Signature(r, reduced, issue121) != sig);
        Result below = r;
        below.verdict = Verdict::AreaBelowAll;
        assert(AreaCheck::Signature(below, pass, issue121) != sig);
        Pass bigger = pass;
        bigger.area = {512, 320};
        assert(AreaCheck::Signature(r, bigger, issue121) != sig);
    }

    // 11. The reporter: a signature prints once, at most 16 severe and 16 mild reports, each
    // followed by one notice, so mild reports in the menus cannot use up the budget before the
    // first severe one; one capture per run, on the first severe verdict.
    {
        Result severe{};
        severe.verdict = Verdict::MixedSizes;
        severe.high = true;
        Result mild{};
        mild.verdict = Verdict::MixedSizes;
        Reporter reporter;
        auto d = reporter.Note(severe, 100, true);
        assert(d.print && !d.suppressed_notice && d.capture == Reporter::Capture::Verdict);
        assert(reporter.CaptureLine(d.capture, severe) ==
               "GPU: area check: frame capture requested (mixed-sizes on a scene pass)");
        d = reporter.Note(severe, 100, true);
        assert(!d.print && !d.suppressed_notice && d.capture == Reporter::Capture::None);
        for (std::uint64_t sig = 1; sig <= Reporter::kMaxReports; ++sig) {
            d = reporter.Note(mild, sig, false);
            assert(d.print && !d.suppressed_notice && d.capture == Reporter::Capture::None);
        }
        d = reporter.Note(mild, 5, false);
        assert(!d.print && !d.suppressed_notice);
        d = reporter.Note(mild, 200, false);
        assert(!d.print && d.suppressed_notice && !d.severe);
        assert(Reporter::SuppressedLine(false) ==
               "GPU: area check: 16 mild reports printed, further mild anomalies are not");
        d = reporter.Note(mild, 100, true);
        assert(!d.print && !d.suppressed_notice);
        // The severe budget is untouched by the mild reports.
        d = reporter.Note(severe, 201, true);
        assert(d.print && !d.suppressed_notice && d.severe &&
               d.capture == Reporter::Capture::None);
        for (std::uint64_t sig = 202; sig < 202 + Reporter::kMaxReports - 2; ++sig) {
            assert(reporter.Note(severe, sig, true).print);
        }
        d = reporter.Note(severe, 300, true);
        assert(!d.print && d.suppressed_notice && d.severe);
        assert(Reporter::SuppressedLine(true) ==
               "GPU: area check: 16 severe reports printed, further severe anomalies are not");
        d = reporter.Note(severe, 301, true);
        assert(!d.print && !d.suppressed_notice);
        // Ok results never print.
        assert(!reporter.Note(Result{}, 0, false).print);

        Result below{};
        below.verdict = Verdict::AreaBelowAll;
        below.high = true;
        assert(reporter.CaptureLine(Reporter::Capture::Verdict, below) ==
               "GPU: area check: frame capture requested (area-below-all)");
    }

    // 13. Without a severe verdict the capture comes at the 3000th scene pass, so every test run
    // brings a census of one 3D frame.
    {
        Reporter reporter;
        for (int i = 0; i < 10; ++i) {
            assert(reporter.Note(Result{}, 0, false).capture == Reporter::Capture::None);
        }
        Result mild{};
        mild.verdict = Verdict::MixedSizes;
        assert(reporter.Note(mild, 7, false).capture == Reporter::Capture::None);
        for (std::uint32_t i = 1; i < Reporter::kCaptureScenePass; ++i) {
            assert(reporter.Note(Result{}, 0, true).capture == Reporter::Capture::None);
        }
        const auto d = reporter.Note(Result{}, 0, true);
        assert(d.capture == Reporter::Capture::ScenePass && !d.print);
        assert(reporter.CaptureLine(d.capture, Result{}) ==
               "GPU: area check: frame capture requested (scene pass 3000, no severe verdict; 1 "
               "of 3011 checked passes flagged)");
        assert(reporter.Note(Result{}, 0, true).capture == Reporter::Capture::None);
        Result severe{};
        severe.verdict = Verdict::AreaBelowAll;
        severe.high = true;
        assert(reporter.Note(severe, 9, true).capture == Reporter::Capture::None);

        // A severe verdict first: no second capture at the 3000th scene pass.
        Reporter early;
        assert(early.Note(severe, 9, false).capture == Reporter::Capture::Verdict);
        for (std::uint32_t i = 0; i < Reporter::kCaptureScenePass + 10; ++i) {
            assert(early.Note(Result{}, 0, true).capture == Reporter::Capture::None);
        }
    }

    std::puts("area check OK");
    return EXIT_SUCCESS;
}

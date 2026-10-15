// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: render-area check of the issue #121 test build. On two RDNA2 machines with 2024 AMD
// drivers the opaque scene only appears in a 512x256 box at the top left. A pass's render area
// is the minimum of its attachments' sizes at offset 0,0 (BeginRenderingFull in
// vk_rasterizer.cpp, vk_scheduler.cpp), so one attachment of that size, or an area set from
// one, draws exactly that box. Rasterizer::CheckRenderArea compares each pass's area with the
// extent of the view really bound for each attachment (a scene proxy, an upscaler output image
// or the guest image, never the guest size alone, so legitimate overrides are not flagged),
// logs what does not fit and requests one frame capture per run.

#pragma once

#include <array>
#include <cstdint>
#include <cstdio>
#include <span>
#include <string>

namespace Vulkan::AreaCheck {

/// Slack below this many pixels is padding: PS4 tiling pads a 1080-line depth target to 1088.
inline constexpr std::uint32_t kPaddingPx = 64;
/// Bit of the depth target in the masks below; colour buffer N is bit N.
inline constexpr unsigned kDepthBit = 8;
inline constexpr const char* kPrefix = "GPU: area check: ";

struct Extent {
    std::uint32_t width{};
    std::uint32_t height{};
    bool operator==(const Extent&) const = default;
};

inline std::string ToString(Extent e) {
    return std::to_string(e.width) + 'x' + std::to_string(e.height);
}

/// What BeginRenderingFull bound in place of the guest images (0x0: nothing).
struct Override {
    Extent proxy{};          ///< reduced pass: every view is a scene proxy, the area its size
    Extent color_redirect{}; ///< colour 0 redirected to the upscaler's output-size image
    Extent depth_redirect{}; ///< the depth target redirected to the upscaler's output size
    bool rejected{};         ///< the redirect was undone: not every attachment could follow
    bool operator==(const Override&) const = default;
};

inline std::string Name(const Override& o) {
    std::string out;
    const auto add = [&out](const char* part) {
        if (!out.empty()) {
            out += '+';
        }
        out += part;
    };
    if (o.proxy.width != 0) {
        add("reduced");
    }
    if (o.color_redirect.width != 0 || o.depth_redirect.width != 0) {
        add(o.rejected ? "redirect-rejected" : "redirected");
    }
    return out.empty() ? "none" : out;
}

/// One attachment of the pass; `format` is the raw VkFormat, named by the caller (Format).
struct Attachment {
    std::uint64_t address{}; ///< guest address
    std::uint32_t format{};
    Extent native{}; ///< the guest image at the bound mip (the depth target has one level)
    Extent bound{};  ///< the view really bound (Bound)
    std::uint8_t slot{}; ///< colour buffer index, unused for the depth target
    std::uint8_t mip{};
    std::uint8_t samples{};       ///< host sample count
    std::uint8_t guest_samples{}; ///< guest sample count
    bool depth{};
};

constexpr std::uint32_t Bit(const Attachment& a) {
    return 1u << (a.depth ? kDepthBit : a.slot);
}

/// The extent of the view BeginRenderingFull binds. A rejected redirect restores colour 0's
/// previous view (the proxy on a reduced pass) but the depth target's native view, even on a
/// reduced pass.
constexpr Extent Bound(const Override& o, Extent native, bool depth, unsigned slot) {
    const bool redirected =
        depth ? o.depth_redirect.width != 0 : slot == 0 && o.color_redirect.width != 0;
    if (redirected && !o.rejected) {
        return depth ? o.depth_redirect : o.color_redirect;
    }
    if (redirected && depth) {
        return native;
    }
    return o.proxy.width != 0 ? o.proxy : native;
}

/// The pass being checked; the fields after `scene` are only printed.
struct Pass {
    Extent area{}; ///< RenderState width and height
    Override overrides{};
    Extent viewport{}; ///< far edge of the guest viewport in host pixels, 0x0 when unknown
    bool scene{};      ///< five or more colour targets and a depth target (G-buffer)
    std::uint64_t vs_hash{};
    std::uint64_t ps_hash{};
    std::uint32_t mrt_mask{};
    float scale_x{1.0f}; ///< target_scale
    float scale_y{1.0f};
};

/// In order of precedence, the highest last.
enum class Verdict : std::uint8_t {
    Ok,
    MixedSizes,         ///< attachments larger than the area, another one sets it
    ViewportBeyondArea, ///< the guest viewport reaches kPaddingPx or more past the area
    AreaBelowAll,       ///< an attachment is larger and none sets the area
    AreaExceeds,        ///< the area is larger than a view (invalid in Vulkan)
};

inline const char* Name(Verdict verdict) {
    switch (verdict) {
    case Verdict::Ok:
        return "ok";
    case Verdict::MixedSizes:
        return "mixed-sizes";
    case Verdict::ViewportBeyondArea:
        return "viewport-beyond-area";
    case Verdict::AreaBelowAll:
        return "area-below-all";
    case Verdict::AreaExceeds:
        return "area-exceeds";
    }
    return "?";
}

/// Masks of attachments (Bit).
struct Result {
    Verdict verdict{Verdict::Ok};
    std::uint32_t larger_mask{};  ///< kPaddingPx or more larger than the area on a dimension
    std::uint32_t limiter_mask{}; ///< within the padding on a dimension: it sets the area there
    std::uint32_t padded_mask{};  ///< within the padding on both dimensions, not equal
    std::uint32_t smaller_mask{}; ///< smaller than the area on a dimension
    bool high{}; ///< worth the run's automatic frame capture
};

enum class Fit : std::uint8_t { Smaller, Within, Larger };

constexpr Fit FitOf(std::uint32_t size, std::uint32_t area) {
    if (size < area) {
        return Fit::Smaller;
    }
    return size - area < kPaddingPx ? Fit::Within : Fit::Larger;
}

/// Runs on every BeginRenderingFull: no allocation. MixedSizes is severe only on a scene pass
/// (a half-resolution effect with the full-size depth target is not).
constexpr Result Check(const Pass& pass, std::span<const Attachment> attachments) {
    Result r{};
    if (attachments.empty()) {
        return r;
    }
    bool width_set = false;
    bool height_set = false;
    for (const auto& a : attachments) {
        const std::uint32_t bit = Bit(a);
        const Fit w = FitOf(a.bound.width, pass.area.width);
        const Fit h = FitOf(a.bound.height, pass.area.height);
        if (w == Fit::Smaller || h == Fit::Smaller) {
            r.smaller_mask |= bit;
        }
        if (w == Fit::Larger || h == Fit::Larger) {
            r.larger_mask |= bit;
        }
        if (w == Fit::Within || h == Fit::Within) {
            r.limiter_mask |= bit;
        }
        if (w == Fit::Within && h == Fit::Within && a.bound != pass.area) {
            r.padded_mask |= bit;
        }
        width_set |= w == Fit::Within;
        height_set |= h == Fit::Within;
    }
    const bool viewport_beyond = pass.viewport != Extent{} &&
                                 (FitOf(pass.viewport.width, pass.area.width) == Fit::Larger ||
                                  FitOf(pass.viewport.height, pass.area.height) == Fit::Larger);
    if (r.smaller_mask != 0) {
        r.verdict = Verdict::AreaExceeds;
    } else if (r.larger_mask != 0 && !(width_set && height_set)) {
        r.verdict = Verdict::AreaBelowAll;
    } else if (viewport_beyond) {
        r.verdict = Verdict::ViewportBeyondArea;
    } else if (r.larger_mask != 0) {
        r.verdict = Verdict::MixedSizes;
    }
    r.high = r.verdict != Verdict::Ok && (r.verdict != Verdict::MixedSizes || pass.scene);
    return r;
}

/// FNV-1a of the verdict, the area, the override and the attachments (not the shaders): the
/// same broken target drawn by many pipelines is reported once.
constexpr std::uint64_t Signature(const Result& r, const Pass& pass,
                                  std::span<const Attachment> attachments) {
    std::uint64_t hash = 14695981039346656037ull;
    const auto mix = [&hash](std::uint64_t value) {
        for (int i = 0; i < 8; ++i) {
            hash ^= (value >> (i * 8)) & 0xff;
            hash *= 1099511628211ull;
        }
    };
    const auto extent = [&mix](Extent e) { mix(std::uint64_t(e.width) << 32 | e.height); };
    mix(std::uint64_t(r.verdict));
    extent(pass.area);
    extent(pass.overrides.proxy);
    extent(pass.overrides.color_redirect);
    extent(pass.overrides.depth_redirect);
    mix(pass.overrides.rejected);
    for (const auto& a : attachments) {
        mix(Bit(a));
        mix(a.address);
        mix(a.format);
        extent(a.native);
        extent(a.bound);
    }
    return hash;
}

/// "c0,c1,d"
inline std::string Masks(std::uint32_t mask) {
    std::string out;
    for (unsigned bit = 0; bit <= kDepthBit; ++bit) {
        if ((mask & (1u << bit)) == 0) {
            continue;
        }
        if (!out.empty()) {
            out += ',';
        }
        out += bit == kDepthBit ? std::string("d") : 'c' + std::to_string(bit);
    }
    return out;
}

/// For the pass's census line in a frame capture (vk_pass_census.h).
inline std::string Short(const Result& r, const Pass& pass) {
    std::string out = Name(r.verdict);
    if (const auto how = Name(pass.overrides); how != "none") {
        out += ' ' + how;
    }
    if (r.verdict == Verdict::Ok) {
        return out;
    }
    if (r.smaller_mask != 0) {
        out += " smaller " + Masks(r.smaller_mask);
    }
    if (r.larger_mask != 0) {
        out += " larger " + Masks(r.larger_mask);
    }
    if (r.verdict == Verdict::MixedSizes) {
        out += " limiter " + Masks(r.limiter_mask);
    }
    if (r.verdict == Verdict::ViewportBeyondArea) {
        out += " viewport " + ToString(pass.viewport);
    }
    return out;
}

/// The log lines of a flagged pass, joined by '\n': the pass, then one per attachment.
/// `format_name(std::uint32_t)` names a raw VkFormat.
template <class FormatName>
std::string Format(const Result& r, const Pass& pass, std::span<const Attachment> attachments,
                   FormatName&& format_name) {
    char tail[128];
    std::snprintf(tail, sizeof(tail), " vs %08llx ps %08llx mrt 0x%x scale %.3fx%.3f",
                  static_cast<unsigned long long>(pass.vs_hash),
                  static_cast<unsigned long long>(pass.ps_hash), pass.mrt_mask,
                  double(pass.scale_x), double(pass.scale_y));
    std::string out = kPrefix;
    out += Name(r.verdict);
    out += " area " + ToString(pass.area) + " override " + Name(pass.overrides) + " viewport " +
           (pass.viewport == Extent{} ? std::string("-") : ToString(pass.viewport)) +
           " scene " + (pass.scene ? '1' : '0') + tail;
    for (const auto& a : attachments) {
        char address[24];
        std::snprintf(address, sizeof(address), "0x%llx",
                      static_cast<unsigned long long>(a.address));
        out += '\n';
        out += kPrefix;
        out += "  " + (a.depth ? std::string("d") : 'c' + std::to_string(a.slot)) + ' ' +
               address + ' ' + std::string(format_name(a.format)) + " native " +
               ToString(a.native) + " bound " + ToString(a.bound) + " mip " +
               std::to_string(a.mip) + " samples " + std::to_string(a.samples) + '/' +
               std::to_string(a.guest_samples);
        const std::uint32_t bit = Bit(a);
        if (r.smaller_mask & bit) {
            out += " smaller";
        }
        if (r.larger_mask & bit) {
            out += " larger";
        }
        if (r.padded_mask & bit) {
            out += " padded";
        }
        if (r.limiter_mask & bit) {
            out += " limiter";
        }
    }
    return out;
}

/// What to print and when to capture: each signature once, at most kMaxReports reports and
/// then one notice; one frame capture per run, on the first severe verdict or, without one, at
/// the kCaptureScenePass-th checked scene pass so every test run brings one 3D frame. A
/// Rasterizer member used by BeginRenderingFull only, never on two threads at once.
class Reporter {
public:
    static constexpr std::uint32_t kMaxReports = 16;
    static constexpr std::uint32_t kCaptureScenePass = 3000;

    enum class Capture : std::uint8_t { None, Verdict, ScenePass };
    struct Decision {
        bool print{};             ///< print Format(): a new signature within the budget
        bool suppressed_notice{}; ///< print SuppressedLine(severe): the first one past the budget
        bool severe{};            ///< which budget: Result::high
        Capture capture{Capture::None};
    };

    /// Every checked pass; `signature` (Signature) is unused for Ok results. Severe and mild
    /// verdicts have separate budgets, so mild ones in the menus never hide the first severe one.
    Decision Note(const Result& r, std::uint64_t signature, bool scene) {
        Decision d{};
        ++checked;
        scene_passes += scene ? 1 : 0;
        if (r.verdict != Verdict::Ok) {
            ++flagged;
            Budget& budget = r.high ? severe_budget : mild_budget;
            d.severe = r.high;
            bool seen_before = false;
            for (std::uint32_t i = 0; i < budget.reports; ++i) {
                seen_before |= budget.seen[i] == signature;
            }
            if (!seen_before && budget.reports < kMaxReports) {
                budget.seen[budget.reports++] = signature;
                d.print = true;
            } else if (!seen_before && !budget.suppressed) {
                budget.suppressed = true;
                d.suppressed_notice = true;
            }
        }
        if (!captured && (r.high || (scene && scene_passes == kCaptureScenePass))) {
            captured = true;
            d.capture = r.high ? Capture::Verdict : Capture::ScenePass;
        }
        return d;
    }

    std::string CaptureLine(Capture capture, const Result& r) const {
        std::string out = std::string(kPrefix) + "frame capture requested (";
        if (capture == Capture::ScenePass) {
            out += "scene pass " + std::to_string(kCaptureScenePass) + ", no severe verdict; " +
                   std::to_string(flagged) + " of " + std::to_string(checked) +
                   " checked passes flagged";
        } else {
            out += Name(r.verdict);
            if (r.verdict == Verdict::MixedSizes) {
                out += " on a scene pass";
            }
        }
        return out + ')';
    }

    static std::string SuppressedLine(bool severe) {
        const char* kind = severe ? "severe" : "mild";
        return std::string(kPrefix) + std::to_string(kMaxReports) + ' ' + kind +
               " reports printed, further " + kind + " anomalies are not";
    }

private:
    struct Budget {
        std::array<std::uint64_t, kMaxReports> seen{};
        std::uint32_t reports{};
        bool suppressed{};
    };
    Budget severe_budget{};
    Budget mild_budget{};
    bool captured{};
    std::uint64_t checked{};
    std::uint64_t flagged{};
    std::uint64_t scene_passes{};
};

} // namespace Vulkan::AreaCheck

// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: host subgroup policy for the game's shaders, written for the PS4's 64-wide waves
// (vk_pipeline_cache.cpp, vk_compute_pipeline.cpp, vk_graphics_pipeline.cpp).
//
// The defaults are exactly shadPS4's: every stage pinned to 64 lanes on a host that has them
// (AMD), no pin elsewhere, LDS barriers only for the drivers shadPS4 lists. The two environment
// variables below exist to test other policies on hardware the maintainers do not have; neither
// changes the game for anyone who does not set it.

#pragma once

#include <cstdint>
#include <cstring>
#include <optional>

namespace Vulkan {

/// GOW3_SUBGROUP_PIN: which stages get a requiredSubgroupSize.
enum class SubgroupPinMode {
    All,     ///< "all" (default): every stage on a 64-lane host, as shadPS4 does.
    Allowed, ///< "allowed": only the stages the driver lists in requiredSubgroupSizeStages.
    None,    ///< "none": no pin at all.
};

inline SubgroupPinMode ParseSubgroupPinMode(const char* env) {
    if (env && std::strcmp(env, "allowed") == 0) {
        return SubgroupPinMode::Allowed;
    }
    if (env && std::strcmp(env, "none") == 0) {
        return SubgroupPinMode::None;
    }
    return SubgroupPinMode::All;
}

/// The subgroup size a shader stage is pinned to with VK_EXT_subgroup_size_control.
///
/// "all" (default) is shadPS4's behaviour: a host with 64-wide subgroups (AMD) pins every stage
/// to 64, the width the shaders were written for, whether or not the stage is in
/// requiredSubgroupSizeStages (AMD lists compute only, so the graphics pins are outside the
/// specification there and the driver is free to ignore them); any other host gets no pin.
///
/// "allowed" pins only the stages the driver lists, to the width the shaders were compiled for:
/// 64 on a 64-lane host, and on a host whose width can vary between `min_size` and `max_size`
/// (Intel: 8 or 16 to 32) `default_size`, the device's reported subgroup size that the recompiler
/// assumed when it lowered wave64 operations. The modules are SPIR-V 1.6, so without that pin the
/// driver may compile a 64-thread workgroup at SIMD16 and split it into four subgroups. A host
/// with one width (NVIDIA, 32) has nothing to pin.
inline std::optional<std::uint32_t> ChoosePinnedSubgroupSize(
    bool size_control, std::uint32_t min_size, std::uint32_t max_size, bool stage_pinnable,
    std::uint32_t default_size, SubgroupPinMode mode = SubgroupPinMode::All) {
    if (!size_control || mode == SubgroupPinMode::None) {
        return std::nullopt;
    }
    if (mode == SubgroupPinMode::All) {
        if (max_size >= 64) {
            return 64u;
        }
        return std::nullopt;
    }
    if (!stage_pinnable) {
        return std::nullopt;
    }
    if (max_size >= 64) {
        return 64u;
    }
    if (min_size >= max_size || default_size < min_size || default_size > max_size) {
        return std::nullopt;
    }
    return default_size;
}

/// Whether the recompiler must add barriers around LDS accesses of 64-thread compute workgroups.
/// On the PS4 such a workgroup is one wave in lockstep and the game's shaders omit the
/// barriers. By default only the drivers shadPS4 lists (NVIDIA, KosmicKrisp) get them.
/// `override` is GOW3_LDS_BARRIERS: "0" or "1" forces the value, anything else is ignored.
/// The recompiled modules are cached without this flag in their key, so a change needs the
/// launcher's "Clear shader cache".
inline bool NeedsLdsBarriers(bool driver_needs_barriers, const char* override) {
    if (override && (override[0] == '0' || override[0] == '1')) {
        return override[0] == '1';
    }
    return driver_needs_barriers;
}

} // namespace Vulkan

// SPDX-License-Identifier: GPL-2.0-or-later
// Host subgroup policy for the game's wave64 shaders (vk_subgroup_policy.h). No game, Vulkan
// device or window required.
#include <cassert>
#include <cstdio>
#include <cstdlib>

#include "video_core/renderer_vulkan/vk_subgroup_policy.h"

using namespace Vulkan;

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    // AMD (32 to 64 lanes, default 64): every stage pinned to 64, whether or not the stage is in
    // requiredSubgroupSizeStages, exactly as before.
    assert(ChoosePinnedSubgroupSize(true, 32, 64, true, 64) == 64u);
    assert(ChoosePinnedSubgroupSize(true, 32, 64, false, 64) == 64u);

    // NVIDIA (always 32): the width cannot vary, nothing to pin; the recompiler's
    // profile.subgroup_size (32) is exact.
    assert(!ChoosePinnedSubgroupSize(true, 32, 32, true, 32));
    assert(!ChoosePinnedSubgroupSize(true, 32, 32, false, 32));

    // Intel Arc (8 or 16 to 32, default 32): compute is pinned to the default the recompiler
    // assumed, so that a SPIR-V 1.6 module is not compiled at SIMD16; fragment and vertex stages
    // are not in requiredSubgroupSizeStages and stay unpinned.
    assert(ChoosePinnedSubgroupSize(true, 8, 32, true, 32) == 32u);
    assert(ChoosePinnedSubgroupSize(true, 16, 32, true, 32) == 32u);
    assert(!ChoosePinnedSubgroupSize(true, 8, 32, false, 32));

    // No subgroup size control, or a default outside the driver's range: nothing.
    assert(!ChoosePinnedSubgroupSize(false, 32, 64, true, 64));
    assert(!ChoosePinnedSubgroupSize(false, 8, 32, true, 32));
    assert(!ChoosePinnedSubgroupSize(true, 8, 16, true, 32));

    // LDS barriers: needed whenever a 64-thread workgroup is not one lockstep subgroup.
    assert(!NeedsLdsBarriers(false, true, nullptr)); // AMD, pinned to 64
    assert(NeedsLdsBarriers(true, true, nullptr));   // driver list (NVIDIA, KosmicKrisp)
    assert(NeedsLdsBarriers(true, false, nullptr));
    assert(NeedsLdsBarriers(false, false, nullptr)); // Intel: 32 lanes at most
    // GOW3_LDS_BARRIERS=0/1 overrides; anything else is ignored.
    assert(!NeedsLdsBarriers(false, false, "0"));
    assert(NeedsLdsBarriers(false, true, "1"));
    assert(!NeedsLdsBarriers(false, true, ""));
    assert(NeedsLdsBarriers(false, false, "x"));
    assert(!NeedsLdsBarriers(false, true, "x"));

    std::puts("subgroup-policy-test: ok");
    return 0;
}

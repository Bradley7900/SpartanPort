// SPDX-License-Identifier: GPL-2.0-or-later
// Host subgroup policy for the game's wave64 shaders (vk_subgroup_policy.h). No game, Vulkan
// device or window required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>

#include "video_core/renderer_vulkan/vk_subgroup_policy.h"

using namespace Vulkan;

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    // Default policy ("all", GOW3_SUBGROUP_PIN unset): exactly what shadPS4 does. A host with
    // 64 lanes (AMD, 32 to 64) pins every stage to 64, whether or not the stage is in
    // requiredSubgroupSizeStages; any other host gets no pin.
    assert(ChoosePinnedSubgroupSize(true, 32, 64, true, 64) == 64u);
    assert(ChoosePinnedSubgroupSize(true, 32, 64, false, 64) == 64u);
    assert(!ChoosePinnedSubgroupSize(true, 32, 32, true, 32));  // NVIDIA: always 32
    assert(!ChoosePinnedSubgroupSize(true, 32, 32, false, 32));
    assert(!ChoosePinnedSubgroupSize(true, 8, 32, true, 32));   // Intel Arc: 8 or 16 to 32
    assert(!ChoosePinnedSubgroupSize(true, 16, 32, true, 32));
    assert(!ChoosePinnedSubgroupSize(true, 8, 32, false, 32));

    // No subgroup size control: nothing, in any mode.
    assert(!ChoosePinnedSubgroupSize(false, 32, 64, true, 64));
    assert(!ChoosePinnedSubgroupSize(false, 8, 32, true, 32, SubgroupPinMode::Allowed));

    // GOW3_SUBGROUP_PIN parsing: "all" (default), "allowed", "none"; anything else is "all".
    assert(ParseSubgroupPinMode(nullptr) == SubgroupPinMode::All);
    assert(ParseSubgroupPinMode("all") == SubgroupPinMode::All);
    assert(ParseSubgroupPinMode("allowed") == SubgroupPinMode::Allowed);
    assert(ParseSubgroupPinMode("none") == SubgroupPinMode::None);
    assert(ParseSubgroupPinMode("garbage") == SubgroupPinMode::All);

    // "allowed": only the stages the driver lists in requiredSubgroupSizeStages, to the width the
    // shaders were compiled for. AMD lists compute only, so fragment and vertex stay unpinned.
    assert(ChoosePinnedSubgroupSize(true, 32, 64, true, 64, SubgroupPinMode::Allowed) == 64u);
    assert(!ChoosePinnedSubgroupSize(true, 32, 64, false, 64, SubgroupPinMode::Allowed));
    // Intel Arc: compute is pinned to the device's reported size, the width the recompiler
    // assumed when it lowered wave64 operations, so that a SPIR-V 1.6 module is not compiled at
    // SIMD16 and split into four subgroups; stages outside the list stay unpinned.
    assert(ChoosePinnedSubgroupSize(true, 8, 32, true, 32, SubgroupPinMode::Allowed) == 32u);
    assert(ChoosePinnedSubgroupSize(true, 16, 32, true, 32, SubgroupPinMode::Allowed) == 32u);
    assert(!ChoosePinnedSubgroupSize(true, 8, 32, false, 32, SubgroupPinMode::Allowed));
    // A host with one width (NVIDIA) has nothing to pin; a reported size outside the driver's
    // range cannot be requested.
    assert(!ChoosePinnedSubgroupSize(true, 32, 32, true, 32, SubgroupPinMode::Allowed));
    assert(!ChoosePinnedSubgroupSize(true, 8, 16, true, 32, SubgroupPinMode::Allowed));

    // "none": no pin anywhere.
    assert(!ChoosePinnedSubgroupSize(true, 32, 64, true, 64, SubgroupPinMode::None));
    assert(!ChoosePinnedSubgroupSize(true, 8, 32, true, 32, SubgroupPinMode::None));

    // LDS barriers: by default only the drivers shadPS4 lists (NVIDIA, KosmicKrisp).
    // GOW3_LDS_BARRIERS=0/1 forces the value; anything else is ignored.
    assert(!NeedsLdsBarriers(false, nullptr)); // AMD, Intel: as shadPS4
    assert(NeedsLdsBarriers(true, nullptr));   // driver list
    assert(NeedsLdsBarriers(false, "1"));      // Intel test: GOW3_LDS_BARRIERS=1
    assert(!NeedsLdsBarriers(true, "0"));
    assert(!NeedsLdsBarriers(false, ""));
    assert(!NeedsLdsBarriers(false, "x"));
    assert(NeedsLdsBarriers(true, "x"));

    // The log prints the mode that was parsed next to the raw variable, so "Allowed" or a
    // trailing space cannot pass for the "allowed" test.
    assert(std::strcmp(SubgroupPinModeName(SubgroupPinMode::All), "all") == 0);
    assert(std::strcmp(SubgroupPinModeName(SubgroupPinMode::Allowed), "allowed") == 0);
    assert(std::strcmp(SubgroupPinModeName(SubgroupPinMode::None), "none") == 0);
    assert(std::strcmp(SubgroupPinModeName(ParseSubgroupPinMode("Allowed")), "all") == 0);

    std::puts("subgroup-policy-test: ok");
    return 0;
}

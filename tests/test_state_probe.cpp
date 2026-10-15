// SPDX-License-Identifier: GPL-2.0-or-later
// GOW3_STATE_PROBE=viewport,depthstruct,depthreemit (vk_state_probe.h): each token changes one
// piece of state the #121 test build suspects a 2024 AMD driver mishandles. No game or Vulkan
// device required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <string>

#include "video_core/renderer_vulkan/vk_state_probe.h"

using namespace Vulkan;

static void SetProbeEnv(const char* value) {
#ifdef _WIN32
    _putenv_s("GOW3_STATE_PROBE", value);
#else
    setenv("GOW3_STATE_PROBE", value, 1);
#endif
}

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    using StateProbe::Flags;
    const Flags none{};
    assert(StateProbe::Parse(nullptr) == none);
    assert(StateProbe::Parse("") == none);
    assert(StateProbe::Parse("0") == none);
    assert(!none.Any());

    const Flags every{.viewport = true, .depthstruct = true, .depthreemit = true};
    assert(StateProbe::Parse("viewport,depthstruct,depthreemit") == every);
    assert(StateProbe::Parse("all") == every);
    assert(StateProbe::Parse("1") == every);
    assert(StateProbe::Parse("ALL") == every);
    assert((StateProbe::Parse("viewport") == Flags{.viewport = true}));
    assert((StateProbe::Parse("depthstruct") == Flags{.depthstruct = true}));
    assert((StateProbe::Parse("depthreemit") == Flags{.depthreemit = true}));
    assert((StateProbe::Parse("bogus,depthreemit") == Flags{.depthreemit = true}));
    assert((StateProbe::Parse("Viewport DepthStruct") ==
            Flags{.viewport = true, .depthstruct = true}));
    assert((StateProbe::Parse(" viewport,,depth") == Flags{.viewport = true}));
    assert(every.Any());
    assert((Flags{.depthstruct = true}.Any()));

    assert(StateProbe::Marker(every) == "GPU: state probe on: viewport depthstruct depthreemit");
    assert((StateProbe::Marker(Flags{.viewport = true}) == "GPU: state probe on: viewport"));
    assert((StateProbe::Marker(Flags{.depthstruct = true, .depthreemit = true}) ==
            "GPU: state probe on: depthstruct depthreemit"));
    assert(StateProbe::Marker(none).empty());

    // Read once: later changes to the variable do not switch a probe mid-run.
    SetProbeEnv("viewport,depthreemit");
    const Flags first = StateProbe::Current();
    assert((first == Flags{.viewport = true, .depthreemit = true}));
    SetProbeEnv("depthstruct");
    assert(StateProbe::Current() == first);

    std::puts("state probe knob OK");
    return EXIT_SUCCESS;
}

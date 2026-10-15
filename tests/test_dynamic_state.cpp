// SPDX-License-Identifier: GPL-2.0-or-later
// DynamicState (vk_scheduler.h): what CommitWith and EmitDepthStencil (GOW3_STATE_PROBE=
// depthreemit, vk_state_probe.h) record. The commands go to fake vkCmd* entry points in the
// default dispatcher; no game or Vulkan device required.
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "video_core/renderer_vulkan/vk_scheduler.h"

VULKAN_HPP_DEFAULT_DISPATCH_LOADER_DYNAMIC_STORAGE

namespace {

std::vector<std::string> calls;

std::string Face(VkStencilFaceFlags face) {
    return face == VK_STENCIL_FACE_FRONT_AND_BACK ? "both"
           : face == VK_STENCIL_FACE_FRONT_BIT    ? "front"
                                                  : "back";
}

void Add(std::string call) {
    calls.push_back(std::move(call));
}

VKAPI_ATTR void VKAPI_CALL DepthTest(VkCommandBuffer, VkBool32 v) {
    Add("depth test " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL DepthWrite(VkCommandBuffer, VkBool32 v) {
    Add("depth write " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL DepthCompare(VkCommandBuffer, VkCompareOp v) {
    Add("depth compare " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL BoundsTest(VkCommandBuffer, VkBool32 v) {
    Add("bounds test " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL Bounds(VkCommandBuffer, float lo, float hi) {
    Add("bounds " + std::to_string(int(lo * 100)) + " " + std::to_string(int(hi * 100)));
}
VKAPI_ATTR void VKAPI_CALL BiasEnable(VkCommandBuffer, VkBool32 v) {
    Add("bias enable " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL Bias(VkCommandBuffer, float k, float clamp, float slope) {
    Add("bias " + std::to_string(int(k)) + " " + std::to_string(int(clamp)) + " " +
        std::to_string(int(slope)));
}
VKAPI_ATTR void VKAPI_CALL StencilTest(VkCommandBuffer, VkBool32 v) {
    Add("stencil test " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL StencilOp(VkCommandBuffer, VkStencilFaceFlags face, VkStencilOp fail,
                                     VkStencilOp pass, VkStencilOp depth_fail, VkCompareOp cmp) {
    Add("stencil op " + Face(face) + " " + std::to_string(fail) + std::to_string(pass) +
        std::to_string(depth_fail) + std::to_string(cmp));
}
VKAPI_ATTR void VKAPI_CALL Reference(VkCommandBuffer, VkStencilFaceFlags face, uint32_t v) {
    Add("reference " + Face(face) + " " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL WriteMask(VkCommandBuffer, VkStencilFaceFlags face, uint32_t v) {
    Add("write mask " + Face(face) + " " + std::to_string(v));
}
VKAPI_ATTR void VKAPI_CALL CompareMask(VkCommandBuffer, VkStencilFaceFlags face, uint32_t v) {
    Add("compare mask " + Face(face) + " " + std::to_string(v));
}

void InstallFakes() {
    auto& d = VULKAN_HPP_DEFAULT_DISPATCHER;
    d.vkCmdSetDepthTestEnable = DepthTest;
    d.vkCmdSetDepthWriteEnable = DepthWrite;
    d.vkCmdSetDepthCompareOp = DepthCompare;
    d.vkCmdSetDepthBoundsTestEnable = BoundsTest;
    d.vkCmdSetDepthBounds = Bounds;
    d.vkCmdSetDepthBiasEnable = BiasEnable;
    d.vkCmdSetDepthBias = Bias;
    d.vkCmdSetStencilTestEnable = StencilTest;
    d.vkCmdSetStencilOp = StencilOp;
    d.vkCmdSetStencilReference = Reference;
    d.vkCmdSetStencilWriteMask = WriteMask;
    d.vkCmdSetStencilCompareMask = CompareMask;
}

int fake_buffer;

auto Runner() {
    calls.clear();
    const vk::CommandBuffer cmdbuf{reinterpret_cast<VkCommandBuffer>(&fake_buffer)};
    return [cmdbuf](const auto& command) { command(cmdbuf); };
}

/// The commands CommitWith records for depth and stencil only (no other state is dirty).
std::vector<std::string> Commit(Vulkan::DynamicState& state) {
    state.CommitWith(true, false, false, Runner());
    return calls;
}

bool Has(const std::vector<std::string>& list, const std::string& call) {
    for (const auto& c : list) {
        if (c == call) {
            return true;
        }
    }
    return false;
}

} // namespace

int main() {
#ifdef _WIN32
    _set_error_mode(_OUT_TO_STDERR);
#endif
    using Vulkan::DynamicState;
    using Vulkan::StencilOps;
    InstallFakes();

    const StencilOps ops{.fail_op = vk::StencilOp::eReplace,
                         .pass_op = vk::StencilOp::eKeep,
                         .depth_fail_op = vk::StencilOp::eZero,
                         .compare_op = vk::CompareOp::eAlways};
    const StencilOps back_ops{.fail_op = vk::StencilOp::eKeep,
                              .pass_op = vk::StencilOp::eKeep,
                              .depth_fail_op = vk::StencilOp::eKeep,
                              .compare_op = vk::CompareOp::eLess};

    // CommitWith: both faces dirty and equal go out as one command, otherwise per face.
    // Static: AnyDirty compares the flags' padding bits too, which only static storage zeroes.
    static DynamicState state{};
    state.SetStencilTestEnabled(true);
    state.SetStencilOps(ops, ops);
    state.SetStencilReferences(3, 4);
    state.SetStencilWriteMasks(0xff, 0xff);
    state.SetStencilCompareMasks(0x0f, 0x0f);
    auto committed = Commit(state);
    assert((committed == std::vector<std::string>{
                             "stencil test 1",
                             "stencil op both 2017",
                             "reference front 3",
                             "reference back 4",
                             "write mask both 255",
                             "compare mask both 15",
                         }));
    assert(!state.AnyDirty());

    // Only the front write mask changed: only that face goes out.
    state.SetStencilWriteMasks(0x7f, 0xff);
    assert((Commit(state) == std::vector<std::string>{"write mask front 127"}));
    // Back ops differ while the front is clean: only the back.
    state.SetStencilOps(ops, back_ops);
    assert((Commit(state) == std::vector<std::string>{"stencil op back 0001"}));

    // With the stencil test off, CommitWith keeps face changes pending.
    state.SetStencilTestEnabled(false);
    state.SetStencilReferences(5, 5);
    assert((Commit(state) == std::vector<std::string>{"stencil test 0"}));
    assert(!state.AnyDirty());
    state.SetStencilTestEnabled(true);
    assert((Commit(state) == std::vector<std::string>{"stencil test 1", "reference both 5"}));

    // EmitDepthStencil: everything, clean or not and whatever the tests, flags left alone.
    static DynamicState probe{};
    probe.SetDepthTestEnabled(true);
    probe.SetDepthCompareOp(vk::CompareOp::eGreaterOrEqual);
    probe.SetDepthTestEnabled(false);
    probe.SetDepthBoundsTestEnabled(true);
    probe.SetDepthBounds(0.25f, 0.75f);
    probe.SetDepthBiasEnabled(true);
    probe.SetDepthBias(2.f, 1.f, 4.f);
    probe.SetStencilTestEnabled(true);
    probe.SetStencilOps(ops, back_ops);
    probe.SetStencilReferences(7, 7);
    probe.SetStencilWriteMasks(0xff, 0x0f);
    probe.SetStencilCompareMasks(1, 1);
    Commit(probe);
    assert(!probe.AnyDirty());
    probe.SetDepthBoundsTestEnabled(false);
    probe.SetDepthBiasEnabled(false);
    probe.SetStencilTestEnabled(false);
    const auto pending = probe.dirty_state;

    probe.EmitDepthStencil(true, Runner());
    const std::vector<std::string> everything{
        "depth test 0",
        "depth write 0",
        "depth compare 6",
        "bounds test 0",
        "bounds 25 75",
        "bias enable 0",
        "bias 2 1 4",
        "stencil test 0",
        "stencil op front 2017",
        "stencil op back 0001",
        "reference both 7",
        "write mask front 255",
        "write mask back 15",
        "compare mask both 1",
    };
    assert(calls == everything);
    assert(std::memcmp(&pending, &probe.dirty_state, sizeof(pending)) == 0);

    // Without depth bounds support the pipelines have no depth bounds dynamic state.
    probe.EmitDepthStencil(false, Runner());
    assert(calls.size() == everything.size() - 2);
    assert(!Has(calls, "bounds test 0") && !Has(calls, "bounds 25 75"));

    std::puts("dynamic state OK");
    return EXIT_SUCCESS;
}

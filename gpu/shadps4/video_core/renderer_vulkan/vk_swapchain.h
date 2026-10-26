// SPDX-FileCopyrightText: Copyright 2024 shadPS4 Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <mutex>
#include <vector>
#include "common/types.h"
#include "video_core/renderer_vulkan/vk_common.h"

namespace Frontend {
class WindowSDL;
}

namespace Vulkan {

class Instance;
class Scheduler;

class Swapchain {
public:
    explicit Swapchain(const Instance& instance, const Frontend::WindowSDL& window);
    ~Swapchain();

    /// Creates (or recreates) the swapchain with a given size.
    void Create(u32 width, u32 height);

    /// Recreates the swapchain with a given size and current surface.
    void Recreate(u32 width, u32 height);

    /// Acquires the next image in the swapchain.
    bool AcquireNextImage();

    /// Presents the current image and move to the next one
    bool Present();

    vk::SurfaceKHR GetSurface() const {
        return surface;
    }

    vk::Image Image() const {
        return (image_index < images.size()) ? images[image_index] : vk::Image{};
    }

    vk::ImageView ImageView() const {
        return (image_index < images_view.size()) ? images_view[image_index] : vk::ImageView{};
    }

    vk::SurfaceFormatKHR GetSurfaceFormat() const {
        return surface_format;
    }

    vk::SwapchainKHR GetHandle() const {
        return swapchain;
    }

    u32 GetWidth() const {
        return width;
    }

    u32 GetHeight() const {
        return height;
    }

    u32 GetImageCount() const {
        return image_count;
    }

    u32 GetFrameIndex() const {
        return frame_index;
    }

    vk::Extent2D GetExtent() const {
        return extent;
    }

    [[nodiscard]] vk::Semaphore GetImageAcquiredSemaphore() const {
        return (frame_index < image_acquired.size()) ? image_acquired[frame_index] : vk::Semaphore{};
    }

    [[nodiscard]] vk::Semaphore GetPresentReadySemaphore() const {
        return (image_index < present_ready.size()) ? present_ready[image_index] : vk::Semaphore{};
    }

    bool HasHDR() const {
        return supports_hdr;
    }

    void SetHDR(bool hdr);

    /// spartanport: VSync from the in-game menu (SpartanSettings::vsync); recreates the swapchain when it
    /// changed. Present thread only.
    void UpdateVsync();

    bool GetHDR() const {
        return needs_hdr;
    }

    /// fp32-gcn1: vblank pacing (driver.cpp). While on, Present() waits until each frame reaches the
    /// screen (VK_KHR_present_wait) and records when, so the frame limiter can time the next frame
    /// from the real refresh instead of its own clock. Returns whether pacing is on: never without
    /// the extension or with Immediate presents (VSync off without Mailbox), which show mid-refresh.
    static bool SetDisplayPacing(bool on);

    /// fp32-gcn1: steady_clock nanoseconds when the last paced frame reached the screen, 0 if none.
    static s64 LastDisplayNs();

    /// fp32-gcn1: even pacing (driver.cpp). With Fifo, each game frame is presented this many times
    /// (one refresh each), so a 30 cap on a 60 Hz screen shows every frame for exactly 2 refreshes.
    /// 0 or 1 = off. FifoActive: whether the present mode is Fifo (VSync on).
    static void SetEvenRepeats(u32 repeats);
    static u32 EvenRepeats();
    static bool FifoActive();

    /// fp32-gcn1: id of the last present (0 without VK_KHR_present_wait), and a bounded wait until
    /// that present reached the screen.
    u64 LastPresentId() const {
        return last_present_id;
    }
    void WaitPresented(u64 id);

private:
    /// Selects the best available swapchain image format
    void FindPresentFormat();

    /// Selects the best available present mode
    void FindPresentMode();

    /// Sets the surface properties according to device capabilities
    bool SetSurfaceProperties();

    /// Recreates the Vulkan surface from the window
    void RecreateSurface();

    /// Destroys current swapchain resources
    void Destroy();

    /// Performs creation of image views and framebuffers from the swapchain images
    void SetupImages();

    /// Creates the image acquired and present ready semaphores
    void RefreshSemaphores();

private:
    const Instance& instance;
    const Frontend::WindowSDL& window;
    vk::SwapchainKHR swapchain{};
    vk::SurfaceKHR surface{};
    vk::SurfaceFormatKHR surface_format;
    vk::Format view_format;
    vk::PresentModeKHR present_mode;
    bool vsync = true; ///< the setting present_mode was chosen for
    u64 last_present_id = 0;
    vk::Extent2D extent;
    vk::SurfaceTransformFlagBitsKHR transform;
    vk::CompositeAlphaFlagBitsKHR composite_alpha;
    std::vector<vk::Image> images;
    std::vector<vk::ImageView> images_view;
    std::vector<vk::Semaphore> image_acquired;
    std::vector<vk::Semaphore> present_ready;
    u32 width = 0;
    u32 height = 0;
    u32 image_count = 0;
    u32 image_index = 0;
    u32 frame_index = 0;
    bool needs_recreation = true;
    bool needs_hdr = false;    // The game requested HDR swapchain
    bool supports_hdr = false; // SC supports HDR output
};

} // namespace Vulkan

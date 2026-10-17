// SPDX-License-Identifier: GPL-2.0-or-later
// gow3 (macOS): Linux helpers the renderer uses for thread identity. Force-included into the GPU
// library on macOS only (CMakeLists.txt), like gow3_win_compat.h on Windows.
#pragma once
#ifdef __APPLE__
#include <pthread.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* The low 32 bits of the system-wide thread id: the renderer compares ids, nothing else. */
static inline int gettid(void) {
    uint64_t id = 0;
    pthread_threadid_np(NULL, &id);
    return (int)id;
}
#ifdef __cplusplus
}
#endif
#endif

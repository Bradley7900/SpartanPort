// SPDX-License-Identifier: GPL-2.0-or-later
// gow3: GOW3_VK_DISABLE_EXT, a test tool that turns off optional device extensions at startup
// (vk_instance.cpp), to find which one a driver mishandles on hardware the maintainers do not
// have.
//
// The value is a comma-separated list of extension names, or the word "all" for every optional
// extension. Spaces and tabs around each name are ignored; names are case-sensitive and must
// match exactly. Nothing changes for anyone who does not set the variable. Which extensions are
// required, and so never disabled, is decided in vk_instance.cpp.

#pragma once

#include <algorithm>
#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace Vulkan {

/// The parsed value of GOW3_VK_DISABLE_EXT.
struct ExtensionPolicy {
    bool all{};                     ///< "all": every optional extension is disabled.
    std::vector<std::string> names; ///< Listed names, without duplicates, in input order.

    bool IsDisabled(std::string_view name) const {
        return all || std::find(names.begin(), names.end(), name) != names.end();
    }

    /// True when the variable disables nothing (unset, empty or only blank items).
    bool Empty() const {
        return !all && names.empty();
    }
};

inline ExtensionPolicy ParseExtensionPolicy(const char* env) {
    ExtensionPolicy policy;
    if (!env) {
        return policy;
    }
    std::string_view rest{env};
    while (true) {
        const std::size_t comma = rest.find(',');
        std::string_view item = rest.substr(0, comma);
        const std::size_t first = item.find_first_not_of(" \t");
        if (first != std::string_view::npos) {
            item = item.substr(first, item.find_last_not_of(" \t") - first + 1);
            if (item == "all") {
                policy.all = true;
            } else if (std::find(policy.names.begin(), policy.names.end(), item) ==
                       policy.names.end()) {
                policy.names.emplace_back(item);
            }
        }
        if (comma == std::string_view::npos) {
            break;
        }
        rest.remove_prefix(comma + 1);
    }
    return policy;
}

/// The listed names that were not applied (unknown, required or unsupported), in input order,
/// for the log. "all" lists nothing, so it has nothing to report.
inline std::vector<std::string> IgnoredNames(const ExtensionPolicy& policy,
                                             const std::vector<std::string>& applied) {
    std::vector<std::string> ignored;
    if (policy.all) {
        return ignored;
    }
    for (const std::string& name : policy.names) {
        if (std::find(applied.begin(), applied.end(), name) == applied.end()) {
            ignored.push_back(name);
        }
    }
    return ignored;
}

/// The names as the log prints them: "a, b", or "none" for an empty list.
inline std::string JoinNames(const std::vector<std::string>& names) {
    if (names.empty()) {
        return "none";
    }
    std::string joined = names.front();
    for (std::size_t i = 1; i < names.size(); ++i) {
        joined += ", ";
        joined += names[i];
    }
    return joined;
}

} // namespace Vulkan

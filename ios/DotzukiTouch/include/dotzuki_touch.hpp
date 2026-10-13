// SPDX-License-Identifier: MIT OR Apache-2.0
// Platform-independent mobile controls. Coordinates are logical points (pt/dp/vp).
#pragma once
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <map>

namespace dotzuki::touch {
constexpr int Pad = 8;
constexpr int Settings = 9;
struct Rect {
    float x = 0, y = 0, w = 0, h = 0;
    float cx() const { return x + w / 2; }
    float cy() const { return y + h / 2; }
    bool contains(float px, float py, float extra = 0) const {
        return px >= x - extra && py >= y - extra && px <= x + w + extra && py <= y + h + extra;
    }
};
struct Options {
    float scale = 1, inset = 0, lift = 0, opacity = .35f;
    bool mirrored = false, floating = false, haptics = false;
};
struct Control { int id; Rect bounds; };
struct Finger { int control; int bits; };

class Controller {
public:
    Options options;
    Rect safe;
    std::array<Control, 6> controls{};

    // A layout change cancels ownership: a move from an old touch cannot re-arm it.
    void layout(Rect area, Options requested, float viewportHeight = 0) {
        reset();
        safe = area;
        enabled_ = area.w >= 320 && area.h >= 220;
        options = requested;
        options.scale = finiteClamp(options.scale, .85f, 1.2f, 1);
        options.inset = finiteClamp(options.inset, 0, 48, 0);
        options.lift = finiteClamp(options.lift, -48, 48, 0);
        options.opacity = finiteClamp(options.opacity, .2f, .7f, .35f);
        if (!enabled_) { controls = {}; fixedPad_ = {}; return; }
        const float s = std::min(options.scale, std::max(.7f, std::min(area.w / 480, area.h / 300)));
        const float margin = 20;
        // Keep both thumb clusters in their own half, even in compact windows.
        const float inset = std::min(options.inset, std::max(0.f, area.w / 2 - 184 * s - margin));
        // Dialogue grows with the viewport, including on unfolded phones and
        // tablets. Only limit its reserve when a compact window needs room for
        // the thumb clusters below the top controls.
        const float fullHeight = viewportHeight > 0 ? viewportHeight : area.y + area.h;
        const float bottomAvoid = std::max(0.f, fullHeight - area.y - area.h);
        const float bottomMargin = std::max(20.f, std::min(
            std::max(48.f, fullHeight * .22f) - bottomAvoid, area.h - 144 * s - 76));
        const float lift = std::clamp(options.lift, 20 - bottomMargin,
            std::max(0.f, area.h - 144 * s - bottomMargin - 76));
        auto circle = [](float x, float y, float diameter) {
            return Rect{x - diameter / 2, y - diameter / 2, diameter, diameter};
        };
        controls = {{
            {Pad, circle(area.x + margin + inset + 72 * s, area.y + area.h - bottomMargin - lift - 72 * s, 144 * s)},
            {0, circle(area.x + area.w - margin - inset - 38 * s, area.y + area.h - bottomMargin - lift - 94 * s, std::max(56.f, 76 * s))},
            {1, circle(area.x + area.w - margin - inset - 38 * s - std::max(72 * s, 60.f), area.y + area.h - bottomMargin - lift - 38 * s, std::max(48.f, 68 * s))},
            {2, {area.x + area.w - margin - 140, area.y + 12, 64, 48}},
            {3, {area.x + area.w - margin - 64, area.y + 12, 64, 48}},
            {Settings, {area.x + margin, area.y + 12, 48, 48}}
        }};
        if (options.mirrored) {
            for (auto &control : controls) control.bounds.x = area.x * 2 + area.w - control.bounds.x - control.bounds.w;
        }
        fixedPad_ = controls[0].bounds;
    }

    // Event kinds: 0 down, 1 move, 2 up, 3 cancel all. Settings activates on up.
    bool event(int kind, int id, float x, float y) {
        if (kind == 3) { reset(); return false; }
        if (!enabled_ || !std::isfinite(x) || !std::isfinite(y)) return false;
        const uint8_t previousBits = bits();
        if (kind == 0) {
            if (fingers_.count(id)) return false;
            int hit = -1;
            for (const auto &control : controls) {
                if (control.bounds.contains(x, y) &&
                    (control.id != Pad || padOwner_ == -1)) { hit = control.id; break; }
            }
            if (hit == -1 && options.floating && padOwner_ == -1 && floatingZone(x, y)) hit = Pad;
            if (hit == -1) return false;
            if (hit == Pad) {
                padOwner_ = id;
                if (options.floating) {
                    const float r = fixedPad_.w / 2;
                    const float left = options.mirrored ? safe.cx() : safe.x;
                    const float right = options.mirrored ? safe.x + safe.w : safe.cx();
                    controls[0].bounds.x = std::clamp(x, left + r, right - r) - r;
                    controls[0].bounds.y = std::clamp(y, safe.y + 76 + r, safe.y + safe.h - r) - r;
                }
            }
            fingers_[id] = {hit, 0};
        }
        auto found = fingers_.find(id);
        if (found == fingers_.end()) return false;
        auto &finger = found->second;
        if (kind == 2) {
            const bool settings = finger.control == Settings && bounds(Settings).contains(x, y);
            if (finger.control == Pad) { padOwner_ = -1; controls[0].bounds = fixedPad_; }
            fingers_.erase(found);
            return settings;
        }
        if (finger.control == Pad) {
            const auto &pad = controls[0].bounds;
            finger.bits = direction(x - pad.cx(), y - pad.cy(), pad.w / 2, finger.bits);
        } else if (finger.control < 8) {
            // Ownership never jumps to another action or to Start while sliding.
            finger.bits = bounds(finger.control).contains(x, y, 10) ? 1 << finger.control : 0;
        }
        // Preserve taps shorter than one game frame. Only takePresses is called
        // from a game thread; layout, ownership, and held-state reads stay on UI.
        const uint8_t current = bits();
        const uint8_t rising = current & ~previousBits;
        if (rising) {
            uint8_t old = pendingPresses_.load(std::memory_order_relaxed);
            uint8_t next;
            do {
                next = (old | rising) & 0x0f;
                next |= (current & 0xf0) ? (current & 0xf0) : (old & 0xf0);
            } while (!pendingPresses_.compare_exchange_weak(old, next, std::memory_order_relaxed));
        }
        return false;
    }

    uint8_t takePresses() { return pendingPresses_.exchange(0, std::memory_order_relaxed); }
    uint8_t sample(uint8_t held) {
        const uint8_t presses = takePresses();
        // A current direction supersedes a queued turn, never creating a diagonal.
        return held | (presses & ((held & 0xf0) ? 0x0f : 0xff));
    }

    bool accepts(float x, float y) const {
        if (!enabled_) return false;
        for (const auto &control : controls) if (control.bounds.contains(x, y)) return true;
        return options.floating && floatingZone(x, y);
    }
    uint8_t bits() const {
        int result = 0;
        for (const auto &entry : fingers_) result |= entry.second.bits;
        return static_cast<uint8_t>(result);
    }
    bool active(int control) const {
        for (const auto &entry : fingers_) if (entry.second.control == control) return true;
        return false;
    }
    void reset() {
        fingers_.clear();
        pendingPresses_.store(0, std::memory_order_relaxed);
        padOwner_ = -1;
        controls[0].bounds = fixedPad_;
    }
    Rect bounds(int id) const {
        for (const auto &control : controls) if (control.id == id) return control.bounds;
        return {};
    }

private:
    Rect fixedPad_;
    int padOwner_ = -1;
    bool enabled_ = false;
    std::atomic<uint8_t> pendingPresses_{0};
    std::map<int, Finger> fingers_;
    static float finiteClamp(float n, float lo, float hi, float fallback) {
        return std::isfinite(n) ? std::clamp(n, lo, hi) : fallback;
    }
    bool floatingZone(float x, float y) const {
        const auto &p = fixedPad_;
        return safe.contains(x, y) && y > std::max(safe.y + 76, p.cy() - p.h) &&
            std::abs(x - p.cx()) < p.w && (options.mirrored ? x > safe.cx() : x < safe.cx());
    }
    static int direction(float x, float y, float radius, int previous) {
        const float length = std::hypot(x, y);
        if (length < radius * .18f || length > radius * 2.2f) return 0;
        const float ax = std::abs(x), ay = std::abs(y);
        // Keep the previous axis near diagonals, but never retain its old sign.
        const bool horizontal = (previous & 0x30) ? ay <= ax * 1.22f :
            ((previous & 0xc0) ? ax > ay * 1.22f : ax >= ay);
        return horizontal ? (x >= 0 ? 1 << 4 : 1 << 5) : (y >= 0 ? 1 << 7 : 1 << 6);
    }
};
} // namespace dotzuki::touch

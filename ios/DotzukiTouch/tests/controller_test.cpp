#include "dotzuki_touch.hpp"
#include <cassert>
#include <iostream>
#include <limits>
using namespace dotzuki::touch;

int main() {
    Controller c;
    c.layout({44, 0, 756, 369}, {});
    auto pad = c.bounds(Pad);
    auto a = c.bounds(0), b = c.bounds(1), menu = c.bounds(Settings);
    // One thumb moves continuously through sectors without lifting.
    c.event(0, 17, pad.cx() + 40, pad.cy()); assert(c.bits() == 16);
    c.event(1, 17, pad.cx() + 40, pad.cy() - 43); assert(c.bits() == 16);
    c.event(1, 17, pad.cx() + 40, pad.cy() - 52); assert(c.bits() == 64);
    c.event(1, 17, pad.cx(), pad.cy()); assert(c.bits() == 0);
    c.event(1, 17, pad.cx() - 50, pad.cy()); assert(c.bits() == 32);
    // Different pointer IDs (not indices), action + movement, independent release.
    c.event(0, 300, a.cx(), a.cy()); assert(c.bits() == 33);
    c.event(0, 100, b.cx(), b.cy()); assert(c.bits() == 35);
    c.event(2, 300, a.cx(), a.cy()); assert(c.bits() == 34);
    c.event(2, 17, pad.cx(), pad.cy()); assert(c.bits() == 2);
    c.event(2, 100, b.cx(), b.cy()); assert(c.bits() == 0);
    // Two fingers on the same action: releasing one does not release the other.
    c.event(0, 1, a.cx(), a.cy()); c.event(0, 2, a.cx(), a.cy());
    c.event(2, 1, a.cx(), a.cy()); assert(c.bits() == 1);
    // Sliding from A over Start does not open the game menu.
    auto start = c.bounds(3);
    c.event(1, 2, start.cx(), start.cy()); assert(c.bits() == 0);
    c.event(1, 2, a.cx(), a.cy()); assert(c.bits() == 1);
    c.event(3, 0, 0, 0); assert(c.bits() == 0);
    c.event(1, 2, a.cx(), a.cy()); assert(c.bits() == 0);
    // No phantom presses from a touch that began outside the controls.
    c.event(0, 50, 420, 150); c.event(1, 50, a.cx(), a.cy()); assert(c.bits() == 0);
    // The first pad finger owns movement; a second cannot introduce opposite input.
    c.event(0, 1, pad.cx() + 40, pad.cy());
    c.event(0, 2, pad.cx() - 40, pad.cy()); assert(c.bits() == 16);
    c.event(2, 2, pad.cx(), pad.cy()); assert(c.bits() == 16);
    c.event(1, 1, pad.cx() + 200, pad.cy()); assert(c.bits() == 0);
    c.event(1, 1, pad.cx() + 80, pad.cy()); assert(c.bits() == 16);
    c.layout({0, 0, 800, 400}, {}); assert(c.bits() == 0);
    c.event(1, 1, pad.cx() + 40, pad.cy()); assert(c.bits() == 0);
    menu = c.bounds(Settings);
    assert(!c.event(0, 1, menu.cx(), menu.cy()));
    assert(c.event(2, 1, menu.cx(), menu.cy())); assert(c.bits() == 0);
    c.event(0, 1, menu.cx(), menu.cy());
    assert(!c.event(2, 1, menu.cx() + 100, menu.cy()));
    // A down/up pair between two frames is delivered once, not lost or repeated.
    c.reset(); a = c.bounds(0); pad = c.bounds(Pad);
    c.event(0, 1, a.cx(), a.cy()); c.event(2, 1, a.cx(), a.cy());
    assert(c.bits() == 0); assert(c.sample(0) == 1); assert(c.sample(0) == 0);
    c.event(0, 2, pad.cx() + 40, pad.cy());
    c.event(1, 2, pad.cx(), pad.cy() - 50);
    assert(c.sample(c.bits()) == 64); // no queued right+up diagonal
    c.event(1, 2, pad.cx() - 40, pad.cy());
    c.event(2, 2, pad.cx(), pad.cy()); assert(c.sample(0) == 32);
    c.event(0, 3, a.cx(), a.cy()); c.reset(); assert(c.sample(0) == 0);
    // Safe-area geometry across landscape/portrait fallback/tablet/windowed sizes.
    for (auto size : {Rect{44, 0, 724, 354}, Rect{0, 0, 640, 360}, Rect{0, 0, 320, 220},
                      Rect{0, 24, 1024, 720}, Rect{0, 44, 390, 700}, Rect{0, 0, 932, 400}}) {
        for (bool mirror : {false, true}) for (float scale : {.85f, 1.f, 1.2f}) for (float lift : {-48.f, 0.f, 48.f}) {
            Options options; options.mirrored = mirror; options.scale = scale;
            options.inset = 48; options.lift = lift;
            c.layout(size, options);
            for (const auto &control : c.controls) {
                const auto r = control.bounds;
                assert(size.contains(r.x, r.y)); assert(size.contains(r.x + r.w, r.y + r.h));
                assert(r.w >= 48 && r.h >= 48);
                for (const auto &other : c.controls) if (control.id != other.id) {
                    const auto q = other.bounds;
                    if (!(r.x + r.w <= q.x || q.x + q.w <= r.x || r.y + r.h <= q.y || q.y + q.h <= r.y)) {
                        std::cerr << "overlap " << size.w << "x" << size.h << " scale " << scale << " mirror " << mirror << " ids " << control.id << "," << other.id << "\n";
                        assert(false);
                    }
                }
            }
            options.floating = true; c.layout(size, options);
            pad = c.bounds(Pad);
            c.event(0, 11, pad.cx(), pad.cy()); assert(c.bits() == 0);
            const auto origin = c.bounds(Pad);
            c.event(1, 11, origin.cx() + 35, origin.cy()); assert(c.bits() == 16);
            c.event(2, 11, origin.cx(), origin.cy());
            assert(c.bounds(Pad).x == pad.x && c.bounds(Pad).y == pad.y);
        }
    }
    // The runner's bottom dialogue occupies 20% of the viewport, including on
    // unfolded phones/tablets. Default thumb clusters must clear that band.
    for (const auto viewport : {Rect{0, 0, 932, 660}, Rect{0, 0, 1024, 768}, Rect{0, 0, 1366, 1024}}) {
        for (bool mirror : {false, true}) for (float scale : {.85f, 1.f, 1.2f}) {
            for (float bottomInset : {0.f, 34.f}) {
                Options options; options.mirrored = mirror; options.scale = scale;
                c.layout({viewport.x, viewport.y, viewport.w, viewport.h - bottomInset}, options, viewport.h);
                for (int id : {Pad, 0, 1}) {
                    const auto r = c.bounds(id);
                    assert(r.y + r.h <= viewport.h * .8f);
                }
            }
        }
    }
    // The dialogue reserve already includes the bottom system inset.
    c.layout({0, 0, 800, 400}, {}, 400);
    const float withoutBottomInset = c.bounds(Pad).cy();
    c.layout({0, 0, 800, 366}, {}, 400);
    assert(c.bounds(Pad).cy() == withoutBottomInset);
    Options invalid; invalid.scale = std::numeric_limits<float>::quiet_NaN();
    c.layout({0, 0, 800, 400}, invalid); assert(std::isfinite(c.bounds(Pad).w));
    c.layout({0, 0, 100, 100}, {}); c.event(0, 1, 0, 0); assert(c.bits() == 0);
    std::cout << "Touch controller: gestures, ownership, cancellation, settings, floating, and layout passed\n";
}

// SPDX-License-Identifier: MIT OR Apache-2.0
#import <UIKit/UIKit.h>

NS_ASSUME_NONNULL_BEGIN
/// Add above the game view, constrained to its full bounds (not its safe area).
/// All callbacks and methods run on the main thread.
@interface DTZTouchOverlay : UIView
/// A, B, Select, Start, Right, Left, Up, Down are bits 0...7.
/// This callback supplies touch state only; the host merges hardware input.
@property(nonatomic, copy, nullable) void (^onInputChanged)(uint8_t bits);
/// Pause/resume the host while the controls editor is presented.
/// The host must also consider its own application/scene lifecycle.
@property(nonatomic, copy, nullable) void (^onSettingsVisibilityChanged)(BOOL visible);
/// Set false when a host modal owns input, or when a physical controller is active.
@property(nonatomic) BOOL controlsEnabled;
/// Clears every held finger, including canceled gestures. Emits zero immediately.
- (void)releaseAll;
/// Call once per game tick on the main thread to retain taps between frames.
/// Returns held buttons plus new presses since the previous call. Merge hardware separately.
- (uint8_t)consumeInput;
/// Presents native size/position/opacity/handedness/floating/haptic settings.
- (void)presentSettings;
@end
NS_ASSUME_NONNULL_END

// SPDX-License-Identifier: MIT OR Apache-2.0
#import "DotzukiTouchOverlay.h"
#include "dotzuki_touch.hpp"

using namespace dotzuki::touch;
static NSString *const PreferenceKey = @"dotzuki-touch-v1";
static NSString *Text(NSString *en, NSString *zh) {
    return [NSLocale.preferredLanguages.firstObject hasPrefix:@"zh"] ? zh : en;
}
@class DTZTouchSettings;
@interface DTZTouchOverlay () {
    Controller _controller;
    Options _options;
    NSMutableDictionary<NSValue *, NSNumber *> *_touchIDs;
    int _nextID;
    uint8_t _bits;
    CGRect _lastBounds;
    UIEdgeInsets _lastInsets;
    BOOL _editing;
    UIImpactFeedbackGenerator *_feedback;
}
- (void)configure;
- (void)saveOptions;
- (void)finishSettings;
- (void)setOption:(NSInteger)option value:(float)value;
- (float)option:(NSInteger)option;
@end

@interface DTZTouchSettings : UIViewController
@property(nonatomic, weak) DTZTouchOverlay *overlay;
@end

@implementation DTZTouchOverlay
- (instancetype)initWithFrame:(CGRect)frame {
    if ((self = [super initWithFrame:frame])) [self initializeControls];
    return self;
}
- (instancetype)initWithCoder:(NSCoder *)coder {
    if ((self = [super initWithCoder:coder])) [self initializeControls];
    return self;
}
- (void)initializeControls {
    self.backgroundColor = UIColor.clearColor;
    self.opaque = NO;
    self.multipleTouchEnabled = YES;
    _controlsEnabled = YES;
    _touchIDs = [NSMutableDictionary dictionary];
    _feedback = [[UIImpactFeedbackGenerator alloc] initWithStyle:UIImpactFeedbackStyleLight];
    NSDictionary *saved = [NSUserDefaults.standardUserDefaults dictionaryForKey:PreferenceKey];
    if (saved) {
        if (saved[@"scale"]) _options.scale = [saved[@"scale"] floatValue];
        if (saved[@"opacity"]) _options.opacity = [saved[@"opacity"] floatValue];
        _options.inset = [saved[@"inset"] floatValue];
        _options.lift = [saved[@"lift"] floatValue];
        _options.mirrored = [saved[@"mirrored"] boolValue];
        _options.floating = [saved[@"floating"] boolValue];
        _options.haptics = [saved[@"haptics"] boolValue];
    }
    NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
    [center addObserver:self selector:@selector(releaseAll) name:UIApplicationWillResignActiveNotification object:nil];
    [center addObserver:self selector:@selector(windowResigned:) name:UIWindowDidResignKeyNotification object:nil];
    self.accessibilityCustomActions = @[[[UIAccessibilityCustomAction alloc]
        initWithName:Text(@"Adjust touch controls", @"调整触控布局") target:self selector:@selector(accessibilitySettings)]];
}
- (BOOL)accessibilitySettings { [self presentSettings]; return YES; }
- (void)windowResigned:(NSNotification *)note {
    if (note.object == self.window) [self releaseAll];
}
- (void)dealloc { [NSNotificationCenter.defaultCenter removeObserver:self]; }
- (void)didMoveToWindow { [super didMoveToWindow]; [self releaseAll]; [self configure]; }
- (void)setHidden:(BOOL)hidden { if (hidden) [self releaseAll]; [super setHidden:hidden]; }
- (void)setControlsEnabled:(BOOL)enabled {
    _controlsEnabled = enabled;
    if (!enabled) [self releaseAll];
    self.userInteractionEnabled = enabled;
    self.alpha = enabled ? 1 : 0;
}
- (void)layoutSubviews {
    [super layoutSubviews];
    if (!CGRectEqualToRect(_lastBounds, self.bounds) || !UIEdgeInsetsEqualToEdgeInsets(_lastInsets, self.safeAreaInsets)) [self configure];
}
- (void)safeAreaInsetsDidChange { [super safeAreaInsetsDidChange]; [self configure]; }
- (void)configure {
    [self releaseAll];
    _lastBounds = self.bounds;
    _lastInsets = self.safeAreaInsets;
    CGRect area = UIEdgeInsetsInsetRect(self.bounds, self.safeAreaInsets);
    _controller.layout({(float)area.origin.x, (float)area.origin.y,
        (float)area.size.width, (float)area.size.height}, _options, CGRectGetMaxY(self.bounds));
    _options = _controller.options;
    [self setNeedsDisplay];
}
- (void)releaseAll {
    _controller.reset();
    [_touchIDs removeAllObjects];
    _nextID = 0;
    _bits = 0;
    if (self.onInputChanged) self.onInputChanged(0);
    [self setNeedsDisplay];
}
- (uint8_t)consumeInput { return _controller.sample(_controller.bits()); }
- (void)sendTouches:(NSSet<UITouch *> *)touches kind:(int)kind {
    if (!_controlsEnabled || _editing) return;
    BOOL settings = NO;
    for (UITouch *touch in touches) {
        NSValue *key = [NSValue valueWithNonretainedObject:touch];
        if (kind == 0) _touchIDs[key] = @(_nextID++);
        NSNumber *identifier = _touchIDs[key];
        if (!identifier) continue;
        CGPoint point = [touch locationInView:self];
        settings |= _controller.event(kind, identifier.intValue, point.x, point.y);
        if (kind == 2) [_touchIDs removeObjectForKey:key];
    }
    uint8_t next = _controller.bits();
    if (_options.haptics && (next & ~_bits)) [_feedback impactOccurred];
    _bits = next;
    if (self.onInputChanged) self.onInputChanged(next);
    [self setNeedsDisplay];
    if (settings) [self presentSettings];
}
- (void)touchesBegan:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event { [self sendTouches:touches kind:0]; }
- (void)touchesMoved:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event { [self sendTouches:touches kind:1]; }
- (void)touchesEnded:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event { [self sendTouches:touches kind:2]; }
- (void)touchesCancelled:(NSSet<UITouch *> *)touches withEvent:(UIEvent *)event { [self releaseAll]; }
- (BOOL)pointInside:(CGPoint)point withEvent:(UIEvent *)event {
    // Blank areas remain available to the host's direct-touch game UI.
    if (!_controlsEnabled || _editing) return NO;
    return _controller.accepts(point.x, point.y);
}
- (void)drawLabel:(NSString *)label at:(CGPoint)point size:(CGFloat)size pressed:(BOOL)pressed {
    UIColor *color = pressed ? [UIColor colorWithRed:.55 green:.875 blue:1 alpha:1] :
        [UIColor colorWithRed:.94 green:.96 blue:.98 alpha:1];
    NSDictionary *attributes = @{NSFontAttributeName: [UIFont boldSystemFontOfSize:size], NSForegroundColorAttributeName: color};
    CGSize dimensions = [label sizeWithAttributes:attributes];
    [label drawAtPoint:CGPointMake(point.x - dimensions.width / 2, point.y - dimensions.height / 2) withAttributes:attributes];
}
- (void)drawRect:(CGRect)rect {
    for (const auto &control : _controller.controls) {
        const auto b = control.bounds;
        if (b.w <= 0) continue;
        const int id = control.id;
        const BOOL pressed = id == Pad ? (_bits & 240) != 0 : id < 8 && (_bits & (1 << id));
        CGRect frame = CGRectMake(b.x, b.y, b.w, b.h);
        if (id <= 1) frame = CGRectInset(frame, 4, 4);
        UIBezierPath *path = (id == Pad || id <= 1) ? [UIBezierPath bezierPathWithOvalInRect:frame] :
            [UIBezierPath bezierPathWithRoundedRect:frame cornerRadius:16];
        [[UIColor colorWithRed:.067 green:.094 blue:.137 alpha:pressed ? .72 : _options.opacity] setFill];
        [path fill];
        [(pressed ? [UIColor colorWithRed:.55 green:.875 blue:1 alpha:1] : [UIColor colorWithWhite:.92 alpha:.6]) setStroke];
        path.lineWidth = pressed ? 2 : 1.2;
        [path stroke];
        if (id == Pad) {
            NSArray<NSString *> *labels = @[@"→", @"←", @"↑", @"↓"];
            const float dx[] = {.31, -.31, 0, 0}, dy[] = {0, 0, -.31, .31};
            for (int n = 0; n < 4; ++n) [self drawLabel:labels[n]
                at:CGPointMake(b.cx() + dx[n] * b.w, b.cy() + dy[n] * b.h) size:23 pressed:(_bits & (1 << (n + 4))) != 0];
            [[UIColor colorWithWhite:.9 alpha:.45] setFill];
            [[UIBezierPath bezierPathWithOvalInRect:CGRectMake(b.cx() - 4, b.cy() - 4, 8, 8)] fill];
        } else {
            NSString *label = id == 0 ? @"A" : id == 1 ? @"B" : id == 2 ? @"SELECT" : id == 3 ? @"START" : @"⚙";
            [self drawLabel:label at:CGPointMake(b.cx(), b.cy()) size:(id == 2 || id == 3) ? 11 : 24 pressed:pressed];
        }
    }
}
- (void)presentSettings {
    if (_editing || !self.window) return;
    UIResponder *responder = self;
    while (responder && ![responder isKindOfClass:UIViewController.class]) responder = responder.nextResponder;
    UIViewController *presenter = (UIViewController *)responder;
    if (!presenter || presenter.presentedViewController) return;
    [self releaseAll];
    _editing = YES;
    if (self.onSettingsVisibilityChanged) self.onSettingsVisibilityChanged(YES);
    DTZTouchSettings *settings = [DTZTouchSettings new]; settings.overlay = self;
    UINavigationController *navigation = [[UINavigationController alloc] initWithRootViewController:settings];
    navigation.modalPresentationStyle = UIModalPresentationFormSheet;
    [presenter presentViewController:navigation animated:YES completion:nil];
}
- (void)finishSettings {
    if (!_editing) return;
    _editing = NO;
    [self releaseAll];
    if (self.onSettingsVisibilityChanged) self.onSettingsVisibilityChanged(NO);
}
- (void)saveOptions {
    [NSUserDefaults.standardUserDefaults setObject:@{@"scale": @(_options.scale), @"inset": @(_options.inset),
        @"lift": @(_options.lift), @"opacity": @(_options.opacity), @"mirrored": @(_options.mirrored),
        @"floating": @(_options.floating), @"haptics": @(_options.haptics)} forKey:PreferenceKey];
}
- (float)option:(NSInteger)option {
    switch (option) {
        case 0: return _options.scale; case 1: return _options.opacity;
        case 2: return _options.inset; case 3: return _options.lift;
        case 4: return _options.mirrored; case 5: return _options.floating; default: return _options.haptics;
    }
}
- (void)setOption:(NSInteger)option value:(float)value {
    switch (option) {
        case 0: _options.scale = value; break; case 1: _options.opacity = value; break;
        case 2: _options.inset = value; break; case 3: _options.lift = value; break;
        case 4: _options.mirrored = value != 0; break; case 5: _options.floating = value != 0; break;
        case 6: _options.haptics = value != 0; break;
        default: _options = Options{}; break;
    }
    [self configure]; [self saveOptions];
}
@end

@implementation DTZTouchSettings
- (void)viewDidLoad {
    [super viewDidLoad];
    self.title = Text(@"Touch controls", @"触控布局");
    self.view.backgroundColor = UIColor.systemBackgroundColor;
    self.navigationItem.rightBarButtonItem = [[UIBarButtonItem alloc] initWithTitle:Text(@"Done", @"完成") style:UIBarButtonItemStyleDone target:self action:@selector(done)];
    self.navigationItem.leftBarButtonItem = [[UIBarButtonItem alloc] initWithTitle:Text(@"Reset", @"恢复默认") style:UIBarButtonItemStylePlain target:self action:@selector(reset)];
    [self buildOptions];
}
- (void)buildOptions {
    for (UIView *view in self.view.subviews) [view removeFromSuperview];
    UIScrollView *scroll = [UIScrollView new]; scroll.translatesAutoresizingMaskIntoConstraints = NO;
    [self.view addSubview:scroll];
    UIStackView *stack = [UIStackView new]; stack.axis = UILayoutConstraintAxisVertical; stack.spacing = 12;
    stack.translatesAutoresizingMaskIntoConstraints = NO; [scroll addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [scroll.leadingAnchor constraintEqualToAnchor:self.view.safeAreaLayoutGuide.leadingAnchor],
        [scroll.trailingAnchor constraintEqualToAnchor:self.view.safeAreaLayoutGuide.trailingAnchor],
        [scroll.topAnchor constraintEqualToAnchor:self.view.safeAreaLayoutGuide.topAnchor],
        [scroll.bottomAnchor constraintEqualToAnchor:self.view.safeAreaLayoutGuide.bottomAnchor],
        [stack.leadingAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.leadingAnchor constant:24],
        [stack.trailingAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.trailingAnchor constant:-24],
        [stack.topAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.topAnchor constant:24],
        [stack.bottomAnchor constraintEqualToAnchor:scroll.contentLayoutGuide.bottomAnchor constant:-24],
        [stack.widthAnchor constraintEqualToAnchor:scroll.frameLayoutGuide.widthAnchor constant:-48]
    ]];
    NSArray<NSString *> *titles = @[Text(@"Size", @"大小"), Text(@"Opacity", @"不透明度"),
        Text(@"Move inward", @"向内移动"), Text(@"Vertical offset", @"垂直位置"),
        Text(@"Swap sides", @"左右交换"), Text(@"Floating direction pad", @"浮动方向盘"), Text(@"Touch feedback", @"触觉反馈")];
    for (NSInteger i = 0; i < titles.count; ++i) {
        UILabel *label = [UILabel new]; label.text = titles[i]; label.numberOfLines = 0;
        label.font = [UIFont preferredFontForTextStyle:UIFontTextStyleBody]; label.adjustsFontForContentSizeCategory = YES;
        [stack addArrangedSubview:label];
        UIControl *control;
        if (i < 4) {
            UISlider *slider = [UISlider new]; slider.minimumValue = i == 0 ? .85 : i == 1 ? .2 : i == 3 ? -48 : 0;
            slider.maximumValue = i == 0 ? 1.2 : i == 1 ? .7 : 48;
            slider.value = [self.overlay option:i]; control = slider;
        } else { UISwitch *toggle = [UISwitch new]; toggle.on = [self.overlay option:i] != 0; control = toggle; }
        control.tag = i; control.accessibilityLabel = titles[i];
        [control addTarget:self action:@selector(changed:) forControlEvents:UIControlEventValueChanged];
        [stack addArrangedSubview:control];
        if (i < 4) [control.heightAnchor constraintGreaterThanOrEqualToConstant:48].active = YES;
    }
}
- (void)changed:(UIControl *)control {
    float value = [control isKindOfClass:UISlider.class] ? ((UISlider *)control).value : ((UISwitch *)control).on;
    [self.overlay setOption:control.tag value:value];
}
- (void)reset { [self.overlay setOption:-1 value:0]; [self buildOptions]; }
- (void)done { [self dismissViewControllerAnimated:YES completion:nil]; }
- (void)viewDidDisappear:(BOOL)animated { [super viewDidDisappear:animated]; [self.overlay finishSettings]; }
@end

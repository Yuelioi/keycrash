// Bounded native registration/listener fixtures. Only logs the explicit
// selected test combination (F18 or a record-only menu action), not input text.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <poll.h>
#include <unistd.h>

static CGKeyCode key = 79;
static CGEventFlags flags = kCGEventFlagMaskControl | kCGEventFlagMaskAlternate |
    kCGEventFlagMaskShift | kCGEventFlagMaskCommand;
static NSString *mode;
static BOOL drop;
static BOOL droppingKey;
static BOOL rewritingKey;
static BOOL replacingKey;
static CGEventRef replacementDown, replacementUp;

static void emit(NSDictionary *value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    fwrite(data.bytes, 1, data.length, stdout); puts(""); fflush(stdout);
}
static BOOL matches(NSEvent *event) {
    return event.type == NSEventTypeKeyDown && event.keyCode == key &&
        (event.modifierFlags & flags) == flags;
}
static void record(CGEventRef event, NSString *kind) {
    emit(@{@"kind": kind, @"mode": mode, @"pid": @(getpid()),
           @"marker": @(event ? CGEventGetIntegerValueField(event, kCGEventSourceUserData) : 0),
           @"front": @(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier)});
}
static CGEventRef replaceEvent(CGEventTapProxy proxy, CGEventType type, CGEventRef event) {
    (void)type;
    // Allocate keyboard templates outside the synchronous tap callback. A NULL
    // source here can query WindowServer and time out; copying the input event
    // can also retain the original event's serialization identity.
    CGEventRef replacement = CGEventCreateCopy(type==kCGEventKeyDown ? replacementDown:replacementUp);
    if (!replacement) return event;
    CGEventSetFlags(replacement, CGEventGetFlags(event));
    int64_t original = CGEventGetIntegerValueField(event, kCGEventSourceUserData);
    CGEventSetIntegerValueField(replacement, kCGEventSourceUserData,
        [mode hasSuffix:@"repost-marked"] ? original : original ^ 1);
    record(replacement, @"replacement");
    if ([mode hasSuffix:@"-replace"]) return replacement; // Caller releases replacement.
    CGEventTapPostEvent(proxy, replacement); // Starts after this tap; no recursion.
    CFRelease(replacement);
    return NULL;
}
static CGEventRef tapped(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    (void)proxy;
    if ((type == kCGEventKeyDown || type == kCGEventKeyUp) &&
        CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) == key) {
        if (type == kCGEventKeyDown && (CGEventGetFlags(event) & flags) == flags) {
            record(event, @"received");
            if ([mode hasSuffix:@"-replace"] || [mode containsString:@"-repost"]) {
                replacingKey = YES;
                return replaceEvent(proxy, type, event);
            }
            if ([mode hasSuffix:@"-rewrite"]) {
                rewritingKey = YES;
                CGEventSetIntegerValueField(event, kCGEventSourceUserData,
                    CGEventGetIntegerValueField(event, kCGEventSourceUserData) ^ 1);
                record(event, @"mutated");
                return event;
            }
            droppingKey = drop && !context;
            return droppingKey ? NULL : event;
        }
        if (type == kCGEventKeyUp && droppingKey) { droppingKey = NO; return NULL; }
        if (type == kCGEventKeyUp && replacingKey) {
            replacingKey = NO;
            return replaceEvent(proxy, type, event);
        }
        if (type == kCGEventKeyUp && rewritingKey) {
            rewritingKey = NO;
            CGEventSetIntegerValueField(event, kCGEventSourceUserData,
                CGEventGetIntegerValueField(event, kCGEventSourceUserData) ^ 1);
        }
    }
    return event;
}
static OSStatus hotkey(EventHandlerCallRef next, EventRef event, void *context) {
    (void)next; (void)event; (void)context;
    record(NULL, @"received"); return noErr;
}
@interface ActionTarget : NSObject
- (void)trigger:(id)sender;
@end
@implementation ActionTarget
- (void)trigger:(id)sender { (void)sender; record(NSApp.currentEvent.CGEvent, @"received"); }
@end

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) return 64;
        mode = [NSString stringWithUTF8String:argv[1]];
        NSDictionary *menuKeys = @{@"menu-c":@8, @"menu-v":@9, @"menu-f":@3, @"menu-comma":@43};
        if (menuKeys[mode]) { key = [menuKeys[mode] unsignedShortValue];flags = kCGEventFlagMaskCommand; }
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        replacementDown=CGEventCreateKeyboardEvent(NULL,key,true);
        replacementUp=CGEventCreateKeyboardEvent(NULL,key,false);
        if ([mode isEqualToString:@"escape"]) {
            if (!AXIsProcessTrusted()) return 7;
            for (int down = 1; down >= 0; down--) {
                CGEventRef event = CGEventCreateKeyboardEvent(NULL, 53, down);
                CGEventSetFlags(event, 0); CGEventPost(kCGHIDEventTap, event); CFRelease(event);
            }
            return 0;
        }
        EventHotKeyRef registration = NULL;
        EventHandlerRef handler = NULL;
        CFMachPortRef tap = NULL;
        CFRunLoopSourceRef source = NULL;
        CFMachPortRef secondTap = NULL;
        CFRunLoopSourceRef secondSource = NULL;
        id monitor = nil;
        NSWindow *window = nil;
        ActionTarget *target = [ActionTarget new];
        OSStatus status = noErr;
        if ([mode isEqualToString:@"request-permission"]) {
            NSDictionary *options=@{(__bridge NSString *)kAXTrustedCheckOptionPrompt:@YES};
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
        } else if ([mode hasPrefix:@"carbon-"]) {
            EventTypeSpec type = {kEventClassKeyboard, kEventHotKeyPressed};
            status = InstallApplicationEventHandler(hotkey, 1, &type, NULL, &handler);
            if (!status) status = RegisterEventHotKey(key, controlKey | optionKey | shiftKey | cmdKey,
                (EventHotKeyID){'KCmt',1}, GetApplicationEventTarget(),
                [mode isEqualToString:@"carbon-exclusive"] ? kEventHotKeyExclusive : 0, &registration);
        } else if ([mode hasPrefix:@"tap-"]) {
            NSArray *parts = [mode componentsSeparatedByString:@"-"];
            BOOL pair=[mode hasSuffix:@"-pair"] || [mode hasSuffix:@"-disabled"];
            if (parts.count != 3 && ![mode hasSuffix:@"-repost-marked"] && !pair && ![mode hasSuffix:@"-rebuild"]) return 64;
            CGEventTapLocation location = (CGEventTapLocation)[parts[1] intValue];
            drop = [parts[2] isEqualToString:@"drop"];
            tap = CGEventTapCreate(location, kCGHeadInsertEventTap,
                [parts[2] isEqualToString:@"listen"] ? kCGEventTapOptionListenOnly : kCGEventTapOptionDefault,
                CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp), tapped, NULL);
            if (!tap) status = -1;
            else {
                source = CFMachPortCreateRunLoopSource(NULL, tap, 0);
                CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
            }
            if (!status && pair) {
                // A second independently managed tap in the same process, always pass.
                secondTap=CGEventTapCreate(kCGAnnotatedSessionEventTap,kCGHeadInsertEventTap,
                    kCGEventTapOptionDefault,CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp),
                    tapped,(void *)1);
                if (!secondTap) status=-1;
                else {
                    secondSource=CFMachPortCreateRunLoopSource(NULL,secondTap,0);
                    CFRunLoopAddSource(CFRunLoopGetCurrent(),secondSource,kCFRunLoopCommonModes);
                    if ([mode hasSuffix:@"-disabled"]) CGEventTapEnable(secondTap,false);
                }
            }
        } else if ([mode hasPrefix:@"nsevent-global"]) {
            monitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^(NSEvent *e) {
                if (matches(e)) record(e.CGEvent, @"received");
            }];
            if (!monitor) status = -1;
        } else if ([mode hasPrefix:@"nsevent-local"] || [mode hasPrefix:@"menu"] ||
                   [mode isEqualToString:@"foreground"]) {
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
            window = [[NSWindow alloc] initWithContentRect:NSMakeRect(200,200,360,120)
                styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
            window.title = @"KeyCrash shortcut fixture";
            [window makeKeyAndOrderFront:nil];
            [NSApp activateIgnoringOtherApps:YES];
            if ([mode hasPrefix:@"nsevent-local"] || [mode isEqualToString:@"foreground"]) {
                monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent *(NSEvent *e) {
                    if (matches(e)) {
                        record(e.CGEvent, @"received");
                        if ([mode isEqualToString:@"nsevent-local-drop"]) return nil;
                    }
                    return e;
                }];
            } else {
                NSMenu *bar = [NSMenu new];
                NSMenuItem *root = [NSMenuItem new];
                NSMenu *menu = [[NSMenu alloc] initWithTitle:@"Fixture"];
                unichar character = menuKeys[mode] ? [@{@"menu-c":@"c", @"menu-v":@"v", @"menu-f":@"f", @"menu-comma":@","}[mode] characterAtIndex:0] : NSF18FunctionKey;
                NSMenuItem *action = [[NSMenuItem alloc] initWithTitle:@"Record test shortcut"
                    action:@selector(trigger:) keyEquivalent:[NSString stringWithCharacters:&character length:1]];
                action.keyEquivalentModifierMask = (NSEventModifierFlags)flags; action.target = target;
                [menu addItem:action];root.submenu=menu;[bar addItem:root];NSApp.mainMenu=bar;
            }
        } else return 64;
        if ([mode isEqualToString:@"nsevent-global-self"]) {
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
            window = [[NSWindow alloc] initWithContentRect:NSMakeRect(200,200,360,120)
                styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
            window.title = @"KeyCrash global monitor self test";
            [window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
        }
        // Let activation settle before publishing readiness.
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:.25]];
        emit(@{@"kind":@"ready", @"pid":@(getpid()), @"mode":mode, @"status":@(status),
               @"ax":@(AXIsProcessTrusted()), @"bundle":NSBundle.mainBundle.bundleIdentifier ?: @"",
               @"front":@(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier)});
        if (status) return 2;
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:50];
        BOOL rebuilt=NO;
        BOOL grantReported=NO;
        struct pollfd input = {STDIN_FILENO,POLLIN,0};
        while (deadline.timeIntervalSinceNow > 0 && poll(&input,1,0) <= 0) {
            if (!grantReported && [mode isEqualToString:@"request-permission"] && AXIsProcessTrusted()) {
                emit(@{@"kind":@"permission-granted",@"pid":@(getpid()),@"ax":@YES});grantReported=YES;
            }
            if (!rebuilt && [mode hasSuffix:@"-rebuild"] && tap && !CGEventTapIsEnabled(tap)) {
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(),source,kCFRunLoopCommonModes);CFRelease(source);
                CFMachPortInvalidate(tap);CFRelease(tap);
                tap=CGEventTapCreate(kCGSessionEventTap,kCGHeadInsertEventTap,kCGEventTapOptionDefault,
                    CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp),tapped,NULL);
                if (!tap) { source=NULL;break; }
                source=CFMachPortCreateRunLoopSource(NULL,tap,0);
                CFRunLoopAddSource(CFRunLoopGetCurrent(),source,kCFRunLoopCommonModes);
                rebuilt=YES;record(NULL,@"rebuilt");
            }
            NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
                untilDate:[NSDate dateWithTimeIntervalSinceNow:.01] inMode:NSDefaultRunLoopMode dequeue:YES];
            if (event) [NSApp sendEvent:event];
        }
        if (monitor) [NSEvent removeMonitor:monitor];
        if (registration) UnregisterEventHotKey(registration);
        if (handler) RemoveEventHandler(handler);
        if (source) { CFRunLoopRemoveSource(CFRunLoopGetCurrent(),source,kCFRunLoopCommonModes);CFRelease(source); }
        if (tap) { CFMachPortInvalidate(tap);CFRelease(tap); }
        if (secondSource) { CFRunLoopRemoveSource(CFRunLoopGetCurrent(),secondSource,kCFRunLoopCommonModes);CFRelease(secondSource); }
        if (secondTap) { CFMachPortInvalidate(secondTap);CFRelease(secondTap); }
        if (window) [window orderOut:nil];
        if (replacementDown) CFRelease(replacementDown);
        if (replacementUp) CFRelease(replacementUp);
        emit(@{@"kind":@"released", @"pid":@(getpid()), @"mode":mode});
        return 0;
    }
}

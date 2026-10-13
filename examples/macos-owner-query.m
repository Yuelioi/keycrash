// Experimental attribution probe. Explicit post-fixture synthesizes only
// Control+Option+Shift+Command+F18/F19/F20 test shortcuts.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <dlfcn.h>
#include <poll.h>
#include <unistd.h>

static const UInt32 modifiers = controlKey | optionKey | shiftKey | cmdKey;
static int query(pid_t pid, uintptr_t identifier);

static void emit(NSDictionary *record) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:nil];
    fwrite(data.bytes, 1, data.length, stdout);
    putchar('\n');
    fflush(stdout);
}

static OSStatus received(EventHandlerCallRef next, EventRef event, void *context) {
    (void)next;
    (void)context;
    EventHotKeyID identifier = {0, 0};
    OSStatus status = GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID,
                                      NULL, sizeof(identifier), NULL, &identifier);
    emit(@{@"kind": @"received", @"pid": @(getpid()), @"status": @(status),
           @"id": @(identifier.id)});
    return noErr;
}

static int hold(UInt32 key, UInt32 options) {
    EventTypeSpec type = {kEventClassKeyboard, kEventHotKeyPressed};
    EventHandlerRef handler = NULL;
    InstallApplicationEventHandler(received, 1, &type, NULL, &handler);
    EventHotKeyRef ref = NULL;
    EventHotKeyID identifier = {'KCrh', 1};
    OSStatus result = RegisterEventHotKey(key, modifiers, identifier,
                                        GetApplicationEventTarget(), options, &ref);
    emit(@{@"kind": @"ready", @"pid": @(getpid()), @"key": @(key),
           @"status": @(result), @"native_id": @((uintptr_t)ref)});
    if (result != noErr) return 2;
    query(getpid(), (uintptr_t)ref); // Positive control using the actual native ID.
    // Bounded lifetime, including if the driver disappears.
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:60];
    struct pollfd input = {STDIN_FILENO, POLLIN, 0};
    while (deadline.timeIntervalSinceNow > 0) {
        if (poll(&input, 1, 0) > 0) break;
        NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                          untilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]
                                             inMode:NSDefaultRunLoopMode dequeue:YES];
        if (event) [NSApp sendEvent:event];
    }
    result = UnregisterEventHotKey(ref);
    if (handler) RemoveEventHandler(handler);
    emit(@{@"kind": @"released", @"status": @(result)});
    return result == noErr ? 0 : 3;
}

static CGEventRef observe(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    (void)proxy;
    if (type != kCGEventKeyDown && type != kCGEventKeyUp && type != 14) return event;
    int64_t key = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
    if (type != 14 && key != 79 && key != 80 && key != 90) return event;
    NSEvent *native = [NSEvent eventWithCGEvent:event];
    NSMutableDictionary *record = [@{@"kind": @"event", @"type": @(type), @"key": @(key),
        @"target": @(CGEventGetIntegerValueField(event, kCGEventTargetUnixProcessID)),
        @"flags": @(CGEventGetFlags(event)),
        @"marker": @(CGEventGetIntegerValueField(event, kCGEventSourceUserData)),
        @"tapped_pid": @((pid_t)(intptr_t)context)} mutableCopy];
    if (native && native.type == NSEventTypeSystemDefined) {
        record[@"subtype"] = @(native.subtype);
        record[@"data1"] = @(native.data1);
        record[@"data2"] = @(native.data2);
    }
    emit(record);
    return event;
}

static int monitor(pid_t pid) {
    CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp) | (1ULL << 14);
    CFMachPortRef tap = CGEventTapCreate(kCGAnnotatedSessionEventTap, kCGTailAppendEventTap,
                                       kCGEventTapOptionListenOnly, mask, observe, NULL);
    if (!tap) {
        emit(@{@"kind": @"permission", @"listen": @(CGPreflightListenEventAccess()),
               @"post": @(CGPreflightPostEventAccess())});
        return 6;
    }
    CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(NULL, tap, 0);
    CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
    CFMachPortRef processTap = pid > 0 ? CGEventTapCreateForPid(pid, kCGTailAppendEventTap,
        kCGEventTapOptionListenOnly, mask, observe, (void *)(intptr_t)pid) : NULL;
    CFRunLoopSourceRef processSource = processTap ? CFMachPortCreateRunLoopSource(NULL, processTap, 0) : NULL;
    if (processSource) CFRunLoopAddSource(CFRunLoopGetCurrent(), processSource, kCFRunLoopCommonModes);
    emit(@{@"kind": @"monitor-ready", @"process_tap": @(processTap != NULL)});
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 5, false);
    CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
    CFMachPortInvalidate(tap);
    CFRelease(source);
    CFRelease(tap);
    if (processSource) {
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), processSource, kCFRunLoopCommonModes);
        CFRelease(processSource);
    }
    if (processTap) {
        CFMachPortInvalidate(processTap);
        CFRelease(processTap);
    }
    return 0;
}

static int postFixture(UInt32 key) {
    // Only synthetic fixture combinations, never the user's target shortcut.
    if ((key != 79 && key != 80 && key != 90) || !CGPreflightPostEventAccess()) return 7;
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStatePrivate);
    for (int down = 1; down >= 0; down--) {
        CGEventRef event = CGEventCreateKeyboardEvent(source, (CGKeyCode)key, down);
        CGEventSetFlags(event, kCGEventFlagMaskControl | kCGEventFlagMaskAlternate |
                               kCGEventFlagMaskShift | kCGEventFlagMaskCommand |
                               kCGEventFlagMaskSecondaryFn);
        if (!down) CGEventSetFlags(event, 0);
        CGEventSetIntegerValueField(event, kCGEventSourceUserData, 0x4b430000 + key);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
        usleep(20000);
    }
    CFRelease(source);
    emit(@{@"kind": @"posted", @"key": @(key)});
    return 0;
}

static int query(pid_t pid, uintptr_t identifier) {
    // These are undocumented, dynamically resolved, READ-ONLY calls. A symbol
    // being present does not prove that cross-process access works on this OS.
    void *library = dlopen("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_LAZY);
    if (!library) return 4;
    int (*mainConnection)(void) = dlsym(library, "CGSMainConnectionID");
    CGError (*connectionForPSN)(int, const ProcessSerialNumber *, int *) =
        dlsym(library, "CGSGetConnectionIDForPSN");
    CGError (*getHotKey)(int, uintptr_t, unichar *, unichar *, unsigned int *) =
        dlsym(library, "CGSGetHotKey");
    if (!mainConnection || !connectionForPSN || !getHotKey) {
        emit(@{@"kind": @"unsupported", @"reason": @"Missing query symbols"});
        dlclose(library);
        return 4;
    }
    ProcessSerialNumber psn = {0, 0};
    OSStatus status = GetProcessForPID(pid, &psn);
    int connection = 0;
    if (status == noErr) status = connectionForPSN(mainConnection(), &psn, &connection);
    if (status != noErr || connection == 0) {
        emit(@{@"kind": @"error", @"pid": @(pid), @"status": @(status)});
        dlclose(library);
        return 5;
    }
    // On macOS 15.5 the Carbon hotkey reference is the native pointer-sized ID.
    // This checks a cooperating fixture, not an enumeration of other programs.
    unichar options = 0xffff, key = 0xffff;
    unsigned int flags = 0xffffffff;
    CGError result = getHotKey(connection, identifier, &options, &key, &flags);
    emit(@{@"kind": @"query", @"pid": @(pid), @"connection": @(connection),
           @"native_id": @(identifier), @"key": @(key), @"flags": @(flags),
           @"return": @(result)});
    dlclose(library);
    return 0;
}

static bool inspectUI(AXUIElementRef element, NSString *press, int depth, int *budget) {
    if (depth > 12 || --*budget < 0) return false;
    CFTypeRef role = NULL, title = NULL, value = NULL, children = NULL;
    AXUIElementCopyAttributeValue(element, kAXRoleAttribute, &role);
    if (role && CFEqual(role, kAXMenuBarRole)) { CFRelease(role); return false; }
    AXUIElementCopyAttributeValue(element, kAXTitleAttribute, &title);
    AXUIElementCopyAttributeValue(element, kAXValueAttribute, &value);
    emit(@{@"kind": @"ui", @"role": role ? [(__bridge id)role description] : @"",
        @"title": title ? [(__bridge id)title description] : @"",
        @"value": value ? [(__bridge id)value description] : @"", @"depth": @(depth)});
    bool clicked = false;
    if (press && title && [press isEqualToString:(__bridge NSString *)title]) {
        AXError result = AXUIElementPerformAction(element, kAXPressAction);
        emit(@{@"kind": @"pressed", @"status": @(result), @"title": press});
        clicked = result == kAXErrorSuccess;
    }
    AXUIElementCopyAttributeValue(element, kAXChildrenAttribute, &children);
    if (!clicked && children && CFGetTypeID(children) == CFArrayGetTypeID()) {
        for (id child in (__bridge NSArray *)children) {
            if (inspectUI((__bridge AXUIElementRef)child, press, depth + 1, budget) && press) {
                clicked = true;
                break;
            }
        }
    }
    if (role) CFRelease(role);
    if (title) CFRelease(title);
    if (value) CFRelease(value);
    if (children) CFRelease(children);
    return clicked;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc == 3 && strcmp(argv[1], "product-ui") == 0) {
            NSString *binary = [[[NSString stringWithUTF8String:argv[0]] stringByDeletingLastPathComponent]
                stringByAppendingPathComponent:@"keycrash"];
            emit(@{@"kind": @"product-ui", @"pid": @(getpid())});
            execl(binary.fileSystemRepresentation, binary.fileSystemRepresentation, NULL);
            return 70;
        }
        if ((argc == 3 || argc == 4) && strncmp(argv[1], "ui-", 3) == 0) {
            pid_t pid = (pid_t)strtol(argv[2], NULL, 10);
            NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
            if (![app.executableURL.lastPathComponent isEqualToString:@"keycrash"]) return 65;
            if (strcmp(argv[1], "ui-key") == 0 && argc == 4) {
                [app activateWithOptions:NSApplicationActivateIgnoringOtherApps];
                usleep(150000);
                CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStatePrivate);
                for (int down = 1; down >= 0; down--) {
                    CGEventRef event = CGEventCreateKeyboardEvent(source, (CGKeyCode)strtoul(argv[3], NULL, 10), down);
                    CGEventSetFlags(event, 0);
                    CGEventPostToPid(pid, event);
                    CFRelease(event);
                    usleep(30000);
                }
                CFRelease(source);
                return 0;
            }
            AXUIElementRef root = AXUIElementCreateApplication(pid);
            AXUIElementSetAttributeValue(root, CFSTR("AXManualAccessibility"), kCFBooleanTrue);
            AXUIElementSetAttributeValue(root, CFSTR("AXEnhancedUserInterface"), kCFBooleanTrue);
            usleep(200000);
            int budget = 300;
            bool clicked = inspectUI(root, argc == 4 ? [NSString stringWithUTF8String:argv[3]] : nil, 0, &budget);
            CFRelease(root);
            return argc == 4 && !clicked ? 1 : 0;
        }
        if (argc == 5 && strcmp(argv[1], "production") == 0) {
            NSString *binary = [[[NSString stringWithUTF8String:argv[0]] stringByDeletingLastPathComponent]
                stringByAppendingPathComponent:@"owner-production"];
            execl(binary.fileSystemRepresentation, binary.fileSystemRepresentation, argv[2], argv[3], argv[4], NULL);
            return 70;
        }
        if (argc != 3 && argc != 4 && !(argc == 2 && strcmp(argv[1], "authorize") == 0)) return 64;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        if (strcmp(argv[1], "state") == 0) {
            NSMutableArray *held = [NSMutableArray array];
            for (int key = 0; key < 128; key++) {
                if (CGEventSourceKeyState(kCGEventSourceStateHIDSystemState, key)) [held addObject:@(key)];
            }
            emit(@{@"kind": @"state", @"held": held, @"flags": @(CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState))});
            return 0;
        }
        if (argc == 2) {
            bool granted = CGRequestPostEventAccess();
            emit(@{@"kind": @"permission", @"post": @(granted)});
            return granted ? 0 : 7;
        }
        if (strcmp(argv[1], "hold") == 0) return hold((UInt32)strtoul(argv[2], NULL, 10), 0);
        if (strcmp(argv[1], "hold-exclusive") == 0) return hold((UInt32)strtoul(argv[2], NULL, 10), 1);
        if (strcmp(argv[1], "query") == 0 && argc == 4) return query((pid_t)strtol(argv[2], NULL, 10), (uintptr_t)strtoull(argv[3], NULL, 10));
        if (strcmp(argv[1], "monitor") == 0) return monitor((pid_t)strtol(argv[2], NULL, 10));
        if (strcmp(argv[1], "post-fixture") == 0) return postFixture((UInt32)strtoul(argv[2], NULL, 10));
        return 64;
    }
}

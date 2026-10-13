// Experimental read-only attribution probe, not a production owner detector.
// Compile on macOS with Cocoa and Carbon. Never synthesizes keyboard input.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <dlfcn.h>
#include <poll.h>
#include <unistd.h>

static const UInt32 modifiers = controlKey | optionKey | shiftKey | cmdKey;
static int query(pid_t pid);

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

static int hold(UInt32 key) {
    EventTypeSpec type = {kEventClassKeyboard, kEventHotKeyPressed};
    EventHandlerRef handler = NULL;
    InstallApplicationEventHandler(received, 1, &type, NULL, &handler);
    EventHotKeyRef ref = NULL;
    EventHotKeyID identifier = {'KCrh', 1};
    OSStatus result = RegisterEventHotKey(key, modifiers, identifier,
                                        GetApplicationEventTarget(), 0, &ref);
    emit(@{@"kind": @"ready", @"pid": @(getpid()), @"key": @(key),
           @"status": @(result)});
    if (result != noErr) return 2;
    query(getpid()); // Positive control: can this API read even its own keys?
    // Bounded lifetime, including if the driver disappears.
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:60];
    struct pollfd input = {STDIN_FILENO, POLLIN, 0};
    while (deadline.timeIntervalSinceNow > 0) {
        if (poll(&input, 1, 0) > 0) break;
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
    }
    result = UnregisterEventHotKey(ref);
    if (handler) RemoveEventHandler(handler);
    emit(@{@"kind": @"released", @"status": @(result)});
    return result == noErr ? 0 : 3;
}

static CGEventRef observe(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    (void)proxy;
    (void)context;
    if (type != kCGEventKeyDown && type != kCGEventKeyUp && type != 14) return event;
    int64_t key = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
    if (type != 14 && key != 79 && key != 80) return event;
    NSEvent *native = [NSEvent eventWithCGEvent:event];
    NSMutableDictionary *record = [@{@"kind": @"event", @"type": @(type), @"key": @(key),
        @"target": @(CGEventGetIntegerValueField(event, kCGEventTargetUnixProcessID)),
        @"flags": @(CGEventGetFlags(event)),
        @"marker": @(CGEventGetIntegerValueField(event, kCGEventSourceUserData))} mutableCopy];
    if (native && native.type == NSEventTypeSystemDefined) {
        record[@"subtype"] = @(native.subtype);
        record[@"data1"] = @(native.data1);
        record[@"data2"] = @(native.data2);
    }
    emit(record);
    return event;
}

static int monitor(void) {
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
    emit(@{@"kind": @"monitor-ready"});
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 5, false);
    CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
    CFMachPortInvalidate(tap);
    CFRelease(source);
    CFRelease(tap);
    return 0;
}

static int postFixture(UInt32 key) {
    // Only synthetic fixture combinations, never the user's target shortcut.
    if ((key != 79 && key != 80) || !CGPreflightPostEventAccess()) return 7;
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStatePrivate);
    for (int down = 1; down >= 0; down--) {
        CGEventRef event = CGEventCreateKeyboardEvent(source, (CGKeyCode)key, down);
        CGEventSetFlags(event, kCGEventFlagMaskControl | kCGEventFlagMaskAlternate |
                               kCGEventFlagMaskShift | kCGEventFlagMaskCommand);
        CGEventSetIntegerValueField(event, kCGEventSourceUserData, 0x4b437268);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
        usleep(20000);
    }
    CFRelease(source);
    return 0;
}

static int query(pid_t pid) {
    // These are undocumented, dynamically resolved, READ-ONLY calls. A symbol
    // being present does not prove that cross-process access works on this OS.
    void *library = dlopen("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics", RTLD_LAZY);
    if (!library) return 4;
    int (*mainConnection)(void) = dlsym(library, "CGSMainConnectionID");
    CGError (*connectionForPSN)(int, const ProcessSerialNumber *, int *) =
        dlsym(library, "CGSGetConnectionIDForPSN");
    bool (*getHotKey)(int, int, unichar *, unichar *, unsigned int *) =
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
    NSMutableArray *entries = [NSMutableArray array];
    // This deliberately bounded scan only tests fixture registrations. It is
    // NOT an exhaustive inventory: UIDs outside this range would be missed.
    for (int uid = 0; uid < 4096; uid++) {
        unichar options = 0xffff, key = 0xffff;
        unsigned int flags = 0xffffffff;
        bool result = getHotKey(connection, uid, &options, &key, &flags);
        if (key != 0xffff && flags != 0xffffffff) {
            [entries addObject:@{@"uid": @(uid), @"key": @(key), @"flags": @(flags),
                                  @"options": @(options), @"return": @(result)}];
        }
    }
    emit(@{@"kind": @"query", @"pid": @(pid), @"connection": @(connection),
           @"entries": entries});
    dlclose(library);
    return 0;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc != 3) return 64;
        [NSApplication sharedApplication];
        if (strcmp(argv[1], "hold") == 0) return hold((UInt32)strtoul(argv[2], NULL, 10));
        if (strcmp(argv[1], "query") == 0) return query((pid_t)strtol(argv[2], NULL, 10));
        if (strcmp(argv[1], "monitor") == 0) return monitor();
        if (strcmp(argv[1], "post-fixture") == 0) return postFixture((UInt32)strtoul(argv[2], NULL, 10));
        return 64;
    }
}

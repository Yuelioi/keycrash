// Experimental read-only attribution probe, not a production owner detector.
// Compile on macOS with Cocoa and Carbon. Never synthesizes keyboard input.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <dlfcn.h>
#include <poll.h>
#include <unistd.h>

static const UInt32 modifiers = controlKey | optionKey | shiftKey | cmdKey;

static void emit(NSDictionary *record) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:nil];
    fwrite(data.bytes, 1, data.length, stdout);
    putchar('\n');
    fflush(stdout);
}

static int hold(UInt32 key) {
    EventHotKeyRef ref = NULL;
    EventHotKeyID identifier = {'KCrh', 1};
    OSStatus result = RegisterEventHotKey(key, modifiers, identifier,
                                        GetApplicationEventTarget(), 0, &ref);
    emit(@{@"kind": @"ready", @"pid": @(getpid()), @"key": @(key),
           @"status": @(result)});
    if (result != noErr) return 2;
    // Bounded lifetime, including if the driver disappears.
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:60];
    struct pollfd input = {STDIN_FILENO, POLLIN, 0};
    while (deadline.timeIntervalSinceNow > 0) {
        if (poll(&input, 1, 0) > 0) break;
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, true);
    }
    result = UnregisterEventHotKey(ref);
    emit(@{@"kind": @"released", @"status": @(result)});
    return result == noErr ? 0 : 3;
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
        return 64;
    }
}

// Experimental global-chain observation. Never labels a tap PID as an owner.
// Posts only Ctrl+Opt+Shift+Cmd+F18; records only its unique marker.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <unistd.h>

static int64_t marker;
static NSMutableArray *events;
static BOOL interrupted;
static void emit(NSDictionary *value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    fwrite(data.bytes,1,data.length,stdout); puts(""); fflush(stdout);
}
static NSArray *tapList(void) {
    CGEventTapInformation rows[512]; uint32_t count = 0;
    CGError result = CGGetEventTapList(512,rows,&count);
    if (result || count >= 512) return nil;
    NSMutableArray *out = [NSMutableArray array];
    for (uint32_t i=0; i<count; i++) {
        CGEventTapInformation t = rows[i];
        if (!(t.eventsOfInterest & (CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp)))) continue;
        [out addObject:@{@"id":@(t.eventTapID), @"pid":@(t.tappingProcess),
            @"target_pid":@(t.processBeingTapped), @"location":@(t.tapPoint),
            @"options":@(t.options), @"mask":@(t.eventsOfInterest), @"enabled":@(t.enabled),
            @"min_us":@(t.minUsecLatency), @"avg_us":@(t.avgUsecLatency), @"max_us":@(t.maxUsecLatency)}];
    }
    return out;
}
static CGEventRef observe(CGEventTapProxy proxy,CGEventType type,CGEventRef event,void *context) {
    (void)proxy;
    if (type==kCGEventTapDisabledByTimeout || type==kCGEventTapDisabledByUserInput) {
        interrupted=YES;return event;
    }
    if (!event || CGEventGetIntegerValueField(event,kCGEventSourceUserData)!=marker) return event;
    NSInteger slot=(NSInteger)context;
    NSMutableDictionary *record=[@{@"location":@(slot/2), @"position":slot%2 ? @"tail":@"head",
        @"type":@(type), @"key":@(CGEventGetIntegerValueField(event,kCGKeyboardEventKeycode)),
        @"target":@(CGEventGetIntegerValueField(event,kCGEventTargetUnixProcessID))} mutableCopy];
    if (type==14) {
        NSEvent *native=[NSEvent eventWithCGEvent:event];
        if (native) record[@"subtype"]=@(native.subtype);
    }
    [events addObject:record];return event;
}
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];[NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        if (!AXIsProcessTrusted() || IsSecureEventInputEnabled()) return 7;
        for (int k=0;k<128;k++) if (CGEventSourceKeyState(kCGEventSourceStateHIDSystemState,k)) return 8;
        marker=((int64_t)getpid()<<32) ^ (int64_t)([NSDate timeIntervalSinceReferenceDate]*1000000);
        events=[NSMutableArray array];
        CFMachPortRef taps[6]={0};CFRunLoopSourceRef sources[6]={0};
        BOOL ready=YES;
        for (NSInteger i=0;i<6;i++) {
            taps[i]=CGEventTapCreate((CGEventTapLocation)(i/2),
                i%2 ? kCGTailAppendEventTap:kCGHeadInsertEventTap,kCGEventTapOptionListenOnly,
                CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp)|CGEventMaskBit(14),observe,(void *)i);
            if (!taps[i]) {ready=NO;break;}
            sources[i]=CFMachPortCreateRunLoopSource(NULL,taps[i],0);
            if (!sources[i]) {ready=NO;break;}
            CFRunLoopAddSource(CFRunLoopGetCurrent(),sources[i],kCFRunLoopCommonModes);
        }
        NSArray *before=tapList();
        if (ready && before) {
            CGEventSourceRef source=CGEventSourceCreate(kCGEventSourceStatePrivate);
            CGEventRef down=CGEventCreateKeyboardEvent(source,79,true);
            CGEventRef up=CGEventCreateKeyboardEvent(source,79,false);
            if (!source || !down || !up) ready=NO;
            else {
                CGEventSetFlags(down,kCGEventFlagMaskControl|kCGEventFlagMaskAlternate|kCGEventFlagMaskShift|kCGEventFlagMaskCommand|kCGEventFlagMaskSecondaryFn);
                CGEventSetFlags(up,0);
                CGEventSetIntegerValueField(down,kCGEventSourceUserData,marker);
                CGEventSetIntegerValueField(up,kCGEventSourceUserData,marker);
                CGEventPost(kCGHIDEventTap,down);usleep(20000);CGEventPost(kCGHIDEventTap,up);
                NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:1];
                while (deadline.timeIntervalSinceNow>0) CFRunLoopRunInMode(kCFRunLoopDefaultMode,.02,false);
            }
            if (up) CFRelease(up);if (down) CFRelease(down);if (source) CFRelease(source);
        } else ready=NO;
        NSArray *after=tapList();
        for (int i=0;i<6;i++) {
            if (taps[i] && !CGEventTapIsEnabled(taps[i])) interrupted=YES;
            if (sources[i]) {CFRunLoopRemoveSource(CFRunLoopGetCurrent(),sources[i],kCFRunLoopCommonModes);CFRelease(sources[i]);}
            if (taps[i]) {CFMachPortInvalidate(taps[i]);CFRelease(taps[i]);}
        }
        emit(@{@"kind":@"chain", @"pid":@(getpid()), @"ready":@(ready), @"interrupted":@(interrupted),
            @"events":events, @"before":before ?: @[], @"after":after ?: @[], @"lists_ok":@(before && after)});
        return ready && !interrupted && after ? 0:2;
    }
}

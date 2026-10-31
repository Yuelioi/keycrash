// Explicit sequence delivery and marked Carbon receipts; no keyboard recording.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <libproc.h>
#include <time.h>
#include <unistd.h>
#include <mach/mach_time.h>
static mach_timebase_info_data_t sequence_timebase;

uint32_t kc_sequence_clock(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t);
    return (uint32_t)((uint64_t)t.tv_sec*1000+t.tv_nsec/1000000);
}
bool kc_sequence_parent(uint32_t pid) { return (uint32_t)getppid()==pid; }
void kc_sequence_release(void *value) { if(value) CFRelease(value); }
void *kc_sequence_source(void) {
    if(mach_timebase_info(&sequence_timebase)!=KERN_SUCCESS || sequence_timebase.denom==0) return NULL;
    return CGEventSourceCreate(kCGEventSourceStatePrivate);
}
uint64_t kc_sequence_flags(void) { return CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState); }
bool kc_sequence_held(uint32_t key) { return key<128 && CGEventSourceKeyState(kCGEventSourceStateHIDSystemState,(CGKeyCode)key); }
void *kc_sequence_event(void *source,uint32_t key,bool down) {
    @autoreleasepool {
        if(key>=0x10000) {
            NSEvent *event=[NSEvent otherEventWithType:NSEventTypeSystemDefined location:NSZeroPoint
                modifierFlags:down?0xa00:0xb00 timestamp:0 windowNumber:0 context:nil subtype:8
                data1:((NSInteger)(key-0x10000)<<16)|((down?0xa:0xb)<<8) data2:-1];
            CGEventRef raw=event.CGEvent;
            return raw ? (void *)CFRetain(raw):NULL;
        }
        CGEventRef event=CGEventCreateKeyboardEvent(source,(CGKeyCode)key,down);
        if(event && key>=54 && key<=62) CGEventSetType(event,kCGEventFlagsChanged);
        return event;
    }
}
void kc_sequence_post(void *event,uint64_t flags,int64_t marker,uint32_t key,bool down) {
    CGEventSetTimestamp(event,(uint64_t)((__uint128_t)mach_absolute_time()*sequence_timebase.numer/sequence_timebase.denom));
    CGEventSetFlags(event,flags|(key>=0x10000?(down?0xa00:0xb00):0));
    CGEventSetIntegerValueField(event,kCGEventSourceUserData,marker);
    CGEventPost(kCGHIDEventTap,event);
}

typedef struct { uint32_t pid,tick; char path[4096]; } KCSequenceReceipt;
typedef struct {
    CFMachPortRef tap; CFRunLoopSourceRef source; CFRunLoopRef loop;
    int64_t marker; bool active,interrupted;
    uint32_t count,cursor; KCSequenceReceipt receipts[256];
} KCSequenceObserver;
static CGEventRef sequence_observe(CGEventTapProxy proxy,CGEventType type,CGEventRef event,void *context) {
    (void)proxy;
    KCSequenceObserver *state=context;
    if(type==kCGEventTapDisabledByTimeout || type==kCGEventTapDisabledByUserInput) {state->interrupted=true;return event;}
    if(!state->active || !event || type!=14 || CGEventGetIntegerValueField(event,kCGEventSourceUserData)!=state->marker) return event;
    @autoreleasepool {
        if([NSEvent eventWithCGEvent:event].subtype!=6) return event;
        int64_t pid=CGEventGetIntegerValueField(event,kCGEventTargetUnixProcessID);
        if(pid<=0 || pid>INT32_MAX || pid==getpid()) return event;
        if(state->count>=256) {state->interrupted=true;return event;}
        KCSequenceReceipt *receipt=&state->receipts[state->count++];
        receipt->pid=(uint32_t)pid;receipt->tick=kc_sequence_clock();
        if(proc_pidpath((int)pid,receipt->path,sizeof(receipt->path))<=0) receipt->path[0]=0;
        receipt->path[sizeof(receipt->path)-1]=0;
    }
    return event;
}
void *kc_sequence_observer(int64_t marker) {
    KCSequenceObserver *state=calloc(1,sizeof(*state)); if(!state) return NULL;
    state->marker=marker;
    state->tap=CGEventTapCreate(kCGAnnotatedSessionEventTap,kCGTailAppendEventTap,kCGEventTapOptionListenOnly,
        CGEventMaskBit(14),sequence_observe,state);
    if(!state->tap) {free(state);return NULL;}
    state->source=CFMachPortCreateRunLoopSource(NULL,state->tap,0);
    if(!state->source) {CFMachPortInvalidate(state->tap);CFRelease(state->tap);free(state);return NULL;}
    state->loop=CFRunLoopGetCurrent();CFRunLoopAddSource(state->loop,state->source,kCFRunLoopDefaultMode);
    return state;
}
void kc_sequence_activate(void *context) { ((KCSequenceObserver *)context)->active=true; }
void kc_sequence_pump(void *context) {
    KCSequenceObserver *state=context;
    CFRunLoopRunInMode(kCFRunLoopDefaultMode,0.001,false);
    if(!CGEventTapIsEnabled(state->tap)) state->interrupted=true;
}
bool kc_sequence_take(void *context,KCSequenceReceipt *receipt) {
    KCSequenceObserver *state=context;
    if(state->cursor==state->count) return false;
    *receipt=state->receipts[state->cursor++];return true;
}
bool kc_sequence_incomplete(void *context) { return ((KCSequenceObserver *)context)->interrupted; }
void kc_sequence_observer_stop(void *context) {
    KCSequenceObserver *state=context; if(!state) return;
    state->active=false;CFRunLoopRemoveSource(state->loop,state->source,kCFRunLoopDefaultMode);
    CFMachPortInvalidate(state->tap);CFRelease(state->source);CFRelease(state->tap);
    state->source=NULL;state->tap=NULL;
}
void kc_sequence_observer_free(void *context) { free(context); }

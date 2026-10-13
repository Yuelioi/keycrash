// Global-filter diagnostics. Snapshot queries and observations never change a tap.
// Only the separately executed, bounded guardian owns temporary tap mutations.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <libproc.h>
#include <dlfcn.h>
#include <poll.h>
#include <signal.h>
#include <fcntl.h>
#include <unistd.h>
#include <time.h>

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec+t.tv_nsec/1e9; }
static char *json(id value) {
    NSData *data=[NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    if (!data) return NULL;
    char *result=calloc(data.length+1,1); if (result) memcpy(result,data.bytes,data.length); return result;
}
void kc_filter_free(char *value) { free(value); }
static void emit(id value) { char *s=json(value); if (s) { puts(s); fflush(stdout); free(s); } }
static id decode(const char *value) {
    if (!value) return nil;
    return [NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:value] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
}
static BOOL allTaps(CGEventTapInformation *entries,uint32_t *count) {
    return CGGetEventTapList(512,entries,count)==kCGErrorSuccess && *count<512;
}
static NSDictionary *tapInfo(CGEventTapInformation t) {
    return @{@"id":@(t.eventTapID),@"target":@(t.processBeingTapped),@"location":@(t.tapPoint),
        @"options":@(t.options),@"mask":@(t.eventsOfInterest),@"enabled":@(t.enabled)};
}
static BOOL keyboard(CGEventTapInformation t) {
    return t.processBeingTapped==0 && (t.eventsOfInterest & (CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(kCGEventKeyUp)));
}
static BOOL active(NSDictionary *t) { return [t[@"options"] unsignedIntValue]==0 && [t[@"enabled"] boolValue]; }
static NSDictionary *snapshot(pid_t pid) {
    struct proc_bsdinfo info={0}; char path[PROC_PIDPATHINFO_MAXSIZE]={0};
    if (pid<=0 || pid==getpid() || pid==getppid() ||
        proc_pidinfo(pid,PROC_PIDTBSDINFO,0,&info,sizeof(info))!=sizeof(info) || info.pbi_uid!=getuid() ||
        proc_pidpath(pid,path,sizeof(path))<=0) return nil;
    NSString *executable=[NSString stringWithUTF8String:path];
    if (![executable containsString:@".app/"] || [executable hasPrefix:@"/System/"] ||
        [executable hasPrefix:@"/usr/"] || [executable hasPrefix:@"/Library/Apple/"]) return nil;
    // NSRunningApplication can be absent in a worker thread's workspace cache
    // even while the PID and its taps remain valid. Identity must come from
    // proc_pidinfo/proc_pidpath, not the UI run loop's application list.
    NSString *bundlePath=executable.stringByDeletingLastPathComponent;
    while (bundlePath.length>1 && ![bundlePath.pathExtension isEqualToString:@"app"])
        bundlePath=bundlePath.stringByDeletingLastPathComponent;
    NSBundle *bundle=[NSBundle bundleWithPath:bundlePath];
    if (!bundle || [bundle.bundleIdentifier hasPrefix:@"com.apple."]) return nil;
    CGEventTapInformation taps[512]; uint32_t count=0;
    if (!allTaps(taps,&count)) return nil;
    NSMutableArray *rows=[NSMutableArray new];
    for (uint32_t i=0;i<count;i++) if (taps[i].tappingProcess==pid && keyboard(taps[i])) [rows addObject:tapInfo(taps[i])];
    [rows sortUsingComparator:^NSComparisonResult(NSDictionary *a,NSDictionary *b) { return [a[@"id"] compare:b[@"id"]]; }];
    return @{@"pid":@(pid),@"path":executable,@"seconds":@(info.pbi_start_tvsec),@"microseconds":@(info.pbi_start_tvusec),
        @"name":[bundle objectForInfoDictionaryKey:@"CFBundleDisplayName"] ?: [bundle objectForInfoDictionaryKey:@"CFBundleName"] ?: bundlePath.lastPathComponent,@"taps":rows};
}
static BOOL same(NSDictionary *expected,BOOL disabled) {
    if (![expected isKindOfClass:NSDictionary.class] || ![expected[@"pid"] isKindOfClass:NSNumber.class]) return NO;
    NSDictionary *current=snapshot([expected[@"pid"] intValue]);
    if (!current) return NO;
    for (NSString *field in @[@"pid",@"path",@"seconds",@"microseconds"])
        if (![current[field] isEqual:expected[field]]) return NO;
    if (![expected[@"taps"] isKindOfClass:NSArray.class]) return NO;
    NSMutableArray *wanted=[NSMutableArray new];
    for (id t in expected[@"taps"]) {
        if (![t isKindOfClass:NSDictionary.class]) return NO;
        NSMutableDictionary *row=[t mutableCopy];
        if (disabled && active(t)) row[@"enabled"]=@NO;
        [wanted addObject:row];
    }
    return [current[@"taps"] isEqual:wanted];
}
char *kc_filter_candidates(void) {
    @autoreleasepool {
        CGEventTapInformation taps[512]; uint32_t count=0;
        if (!allTaps(taps,&count)) return json(@{@"error":@"list"});
        NSMutableSet *pids=[NSMutableSet new];
        for (uint32_t i=0;i<count;i++) if (keyboard(taps[i]) && !taps[i].options && taps[i].enabled) [pids addObject:@(taps[i].tappingProcess)];
        NSMutableArray *rows=[NSMutableArray new];
        for (NSNumber *pid in [[pids allObjects] sortedArrayUsingSelector:@selector(compare:)]) {
            NSDictionary *row=snapshot(pid.intValue); if (row) [rows addObject:row];
        }
        return json(@{@"candidates":rows});
    }
}
int kc_filter_validate(const char *value,bool disabled) { @autoreleasepool { return same(decode(value),disabled) ? 0:1; } }
static volatile sig_atomic_t stopping;
static void stop(int number) { (void)number; stopping=1; }
int kc_filter_guardian(void) {
    @autoreleasepool {
        signal(SIGTERM,stop); signal(SIGINT,stop); signal(SIGHUP,stop); signal(SIGPIPE,SIG_IGN);
        int flags=fcntl(STDOUT_FILENO,F_GETFL);
        if (flags<0 || fcntl(STDOUT_FILENO,F_SETFL,flags|O_NONBLOCK)<0) return 71;
        // Bounded input handshake; no tap has been changed while awaiting a request.
        char request[65536]; size_t used=0; double handshake=now()+5; BOOL complete=NO;
        while (used<sizeof(request)-1 && now()<handshake && !stopping) {
            struct pollfd p={STDIN_FILENO,POLLIN,0};
            if (poll(&p,1,20)>0) {
                char c; if (read(STDIN_FILENO,&c,1)!=1) return 64;
                if (c=='\n') { complete=YES; break; } request[used++]=c;
            }
        }
        request[used]=0;
        if (!complete) return 64;
        NSDictionary *selected=decode(request);
        [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory]; [NSApp finishLaunching];
        if (!AXIsProcessTrusted() || !same(selected,NO) || stopping) { emit(@{@"kind":@"error",@"error":@"changed"}); return 65; }
        void *lib=dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight",RTLD_LAZY);
        CGError (*enable)(uint32_t,bool)=lib ? dlsym(lib,"SLEventTapIDEnable"):NULL;
        if (!enable) { emit(@{@"kind":@"error",@"error":@"unsupported"}); if (lib) dlclose(lib); return 69; }
        NSMutableArray *changed=[NSMutableArray new]; BOOL ok=YES;
        double deadline=now()+30;
        for (NSDictionary *t in selected[@"taps"]) if (active(t)) {
            struct pollfd p={STDIN_FILENO,POLLIN,0};
            if (stopping || poll(&p,1,0)!=0) { ok=NO; break; }
            [changed addObject:t];
            if (enable([t[@"id"] unsignedIntValue],false)) { ok=NO; break; }
        }
        ok=ok && changed.count>0 && same(selected,YES);
        emit(@{@"kind":ok ? @"ready":@"error",@"error":@"control",@"pid":@(getpid()),@"mutated":@(changed.count>0)});
        NSString *reason=@"control";
        if (ok) {
            reason=@"deadline";
            while (now()<deadline) {
                if (stopping) { reason=@"signal"; break; }
                struct pollfd p={STDIN_FILENO,POLLIN,0};
                int status=poll(&p,1,20);
                if (status>0) { reason=@"released"; break; }
                if (status<0 && !stopping) { reason=@"pipe"; break; }
                if (!same(selected,YES)) { reason=@"changed"; break; }
            }
        }
        // Restore only matching original identities/metadata, even after partial failure.
        BOOL restored=YES;
        for (NSDictionary *t in changed) {
            NSDictionary *current=snapshot([selected[@"pid"] intValue]);
            BOOL identity=current!=nil;
            for (NSString *f in @[@"path",@"seconds",@"microseconds"]) identity &= [current[f] isEqual:selected[f]];
            NSDictionary *found=nil;
            if (identity) for (NSDictionary *c in current[@"taps"]) {
                NSMutableDictionary *a=[c mutableCopy],*b=[t mutableCopy];
                [a removeObjectForKey:@"enabled"]; [b removeObjectForKey:@"enabled"];
                if ([a isEqual:b]) { found=c; break; }
            }
            if (!found || enable([t[@"id"] unsignedIntValue],true)) restored=NO;
        }
        restored=restored && same(selected,NO);
        emit(@{@"kind":@"restored",@"ok":@(restored),@"reason":reason});
        dlclose(lib); return restored ? 0:3;
    }
}

typedef bool (*Cancel)(void *);
typedef struct { int64_t marker; uint32_t mask; bool interrupted; __unsafe_unretained NSMutableSet *pids; } Observation;
typedef struct { Observation *state; int index; } Slot;
static CGEventRef observe(CGEventTapProxy proxy,CGEventType type,CGEventRef event,void *context) {
    (void)proxy; Slot *slot=context; Observation *s=slot->state;
    if (type==kCGEventTapDisabledByTimeout || type==kCGEventTapDisabledByUserInput) { s->interrupted=true; return event; }
    if (!event || CGEventGetIntegerValueField(event,kCGEventSourceUserData)!=s->marker) return event;
    if (type==kCGEventKeyDown) s->mask|=1u<<slot->index;
    if (type==14 && [NSEvent eventWithCGEvent:event].subtype==6) {
        int64_t pid=CGEventGetIntegerValueField(event,kCGEventTargetUnixProcessID);
        if (pid>0 && pid<=INT_MAX) [s->pids addObject:@(pid)];
    }
    return event;
}
char *kc_filter_observe(uint16_t key,uint64_t flags,Cancel cancelled,void *context) {
    @autoreleasepool {
        if (!AXIsProcessTrusted()) return json(@{@"error":@"permission"});
        if (IsSecureEventInputEnabled()) return json(@{@"error":@"secure"});
        for (int k=0;k<128;k++) if (CGEventSourceKeyState(kCGEventSourceStateHIDSystemState,k)) return json(@{@"error":@"keys"});
        NSMutableSet *pids=[NSMutableSet new];
        Observation state={.marker=((int64_t)getpid()<<32) ^ (int64_t)(now()*1e9),.pids=pids};
        CFMachPortRef taps[6]={0}; CFRunLoopSourceRef sources[6]={0}; Slot slots[6]; BOOL ok=YES;
        for (int i=0;i<6;i++) {
            slots[i]=(Slot){&state,i};
            taps[i]=CGEventTapCreate(i/2,i%2,kCGEventTapOptionListenOnly,
                CGEventMaskBit(kCGEventKeyDown)|CGEventMaskBit(14),observe,&slots[i]);
            if (!taps[i]) { ok=NO; break; }
            sources[i]=CFMachPortCreateRunLoopSource(NULL,taps[i],0);
            if (!sources[i]) { ok=NO; break; }
            CFRunLoopAddSource(CFRunLoopGetCurrent(),sources[i],kCFRunLoopDefaultMode);
        }
        if (ok && !cancelled(context)) {
            CGEventSourceRef input=CGEventSourceCreate(kCGEventSourceStatePrivate);
            CGEventRef down=input ? CGEventCreateKeyboardEvent(input,key,true):NULL;
            CGEventRef up=input ? CGEventCreateKeyboardEvent(input,key,false):NULL;
            if (!down || !up) ok=NO;
            else {
                CGEventSetFlags(down,flags); CGEventSetFlags(up,0);
                CGEventSetIntegerValueField(down,kCGEventSourceUserData,state.marker);
                CGEventSetIntegerValueField(up,kCGEventSourceUserData,state.marker);
                if (!cancelled(context)) {
                    CGEventPost(kCGHIDEventTap,down); usleep(20000); CGEventPost(kCGHIDEventTap,up);
                    double deadline=now()+.8;
                    while (now()<deadline && !cancelled(context)) CFRunLoopRunInMode(kCFRunLoopDefaultMode,.02,false);
                }
            }
            if (down) CFRelease(down); if (up) CFRelease(up); if (input) CFRelease(input);
        }
        for (int i=0;i<6;i++) {
            if (taps[i] && !CGEventTapIsEnabled(taps[i])) ok=NO;
            if (sources[i]) { CFRunLoopRemoveSource(CFRunLoopGetCurrent(),sources[i],kCFRunLoopDefaultMode); CFRelease(sources[i]); }
            if (taps[i]) { CFMachPortInvalidate(taps[i]); CFRelease(taps[i]); }
        }
        if (cancelled(context)) return json(@{@"error":@"cancelled"});
        if (!ok || state.interrupted) return json(@{@"error":@"observation"});
        NSMutableArray *owners=[NSMutableArray new];
        for (NSNumber *pid in [[pids allObjects] sortedArrayUsingSelector:@selector(compare:)]) {
            char path[PROC_PIDPATHINFO_MAXSIZE]={0};
            if (proc_pidpath(pid.intValue,path,sizeof(path))>0) [owners addObject:@{@"pid":pid,@"path":[NSString stringWithUTF8String:path]}];
        }
        return json(@{@"mask":@(state.mask),@"receivers":owners});
    }
}

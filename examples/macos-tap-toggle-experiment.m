// Private-API experiment restricted to the exact task-owned fixture executable.
// An exec'd guardian owns the mutation; controller death closes its lease pipe.
#import <Cocoa/Cocoa.h>
#include <libproc.h>
#include <dlfcn.h>
#include <poll.h>
#include <unistd.h>
#include <limits.h>
#include <signal.h>
#include <spawn.h>
#include <sys/wait.h>
#include <time.h>
#include <errno.h>
#include <fcntl.h>
#include <mach-o/dyld.h>

extern char **environ;
static volatile sig_atomic_t stopping;
static void stopExperiment(int number) { (void)number; stopping=1; }
static double monotonic(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t);
    return t.tv_sec + t.tv_nsec/1e9;
}
static void emit(NSDictionary *value) {
    NSData *data=[NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    fwrite(data.bytes,1,data.length,stdout); puts(""); fflush(stdout);
}
typedef struct {
    pid_t pid;
    uint64_t seconds, microseconds;
    CGEventTapInformation tap;
} Target;

static BOOL processIdentity(pid_t pid, NSString *expected, struct proc_bsdinfo *info) {
    char path[PROC_PIDPATHINFO_MAXSIZE]={0};
    return proc_pidinfo(pid,PROC_PIDTBSDINFO,0,info,sizeof(*info))==sizeof(*info) &&
        proc_pidpath(pid,path,sizeof(path))>0 &&
        [[NSString stringWithUTF8String:path] isEqualToString:expected];
}
// Public metadata has no tap-generation token; same-process ID reuse remains a limit.
static BOOL lookup(Target target, NSString *expected, BOOL *enabled) {
    struct proc_bsdinfo info={0};
    if (!processIdentity(target.pid,expected,&info) ||
        info.pbi_start_tvsec!=target.seconds || info.pbi_start_tvusec!=target.microseconds) return NO;
    CGEventTapInformation entries[512]; uint32_t count=0;
    if (CGGetEventTapList(512,entries,&count) || count>=512) return NO;
    for (uint32_t i=0;i<count;i++) {
        CGEventTapInformation t=entries[i], original=target.tap;
        if (t.eventTapID==original.eventTapID && t.tappingProcess==target.pid &&
            t.processBeingTapped==original.processBeingTapped && t.tapPoint==original.tapPoint &&
            t.options==original.options && t.eventsOfInterest==original.eventsOfInterest) {
            *enabled=t.enabled; return YES;
        }
    }
    return NO;
}
static int guard(pid_t pid, pid_t controller, NSString *expected) {
    // A stalled result reader cannot hold up restoration by filling its pipe.
    int outputFlags=fcntl(STDOUT_FILENO,F_GETFL);
    if (outputFlags<0 || fcntl(STDOUT_FILENO,F_SETFL,outputFlags|O_NONBLOCK)<0) return 71;
    struct proc_bsdinfo info={0};
    if (!processIdentity(pid,expected,&info)) return 65;
    [NSApplication sharedApplication]; [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    [NSApp finishLaunching];
    if (!AXIsProcessTrusted()) return 7;
    Target targets[512];
    CGEventTapInformation entries[512]; uint32_t count=0,matched=0;
    if (CGGetEventTapList(512,entries,&count) || count>=512) return 2;
    for (uint32_t i=0;i<count;i++) {
        CGEventTapInformation t=entries[i];
        if (t.tappingProcess==pid && !t.options && t.enabled &&
            (t.eventsOfInterest & CGEventMaskBit(kCGEventKeyDown))) {
            targets[matched++]=(Target){.pid=pid,.seconds=info.pbi_start_tvsec,
                .microseconds=info.pbi_start_tvusec,.tap=t};
        }
    }
    if (!matched) return 65;
    void *lib=dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight",RTLD_LAZY);
    // ABI verified on macOS 15.5 arm64, not a stable public API.
    CGError (*setEnabled)(uint32_t,bool)=lib ? dlsym(lib,"SLEventTapIDEnable"):NULL;
    bool (*isEnabled)(uint32_t)=lib ? dlsym(lib,"SLEventTapIDIsEnabled"):NULL;
    if (!setEnabled || !isEnabled) return 69;
    uint32_t identifier=targets[0].tap.eventTapID;
    struct pollfd lease={STDIN_FILENO,POLLIN,0};
    BOOL enabled=NO;
    // If the controller already vanished, never start a new mutation.
    BOOL queriedBefore=YES;
    for (uint32_t i=0;i<matched;i++) {
        if (stopping || poll(&lease,1,0)!=0 || !lookup(targets[i],expected,&enabled) || !enabled) return 75;
        queriedBefore &= isEnabled(targets[i].tap.eventTapID);
    }
    if (!queriedBefore) return 75;
    double deadline=monotonic()+8;
    CGError disabled=kCGErrorSuccess; uint32_t attempted=0;
    for (uint32_t i=0;i<matched;i++) {
        if (stopping || poll(&lease,1,0)!=0 || !lookup(targets[i],expected,&enabled) || !enabled) {
            disabled=kCGErrorInvalidConnection; break;
        }
        attempted++; // Even a reported API error can have changed external state.
        disabled=setEnabled(targets[i].tap.eventTapID,false);
        if (disabled) break;
    }
    BOOL present=YES,anyEnabled=NO,privateAfter=NO;
    NSMutableArray *ids=[NSMutableArray new];
    for (uint32_t i=0;i<matched;i++) {
        BOOL current=YES; present &= lookup(targets[i],expected,&current); anyEnabled |= current;
        privateAfter |= isEnabled(targets[i].tap.eventTapID);
        [ids addObject:@(targets[i].tap.eventTapID)];
    }
    emit(@{@"kind":@"ready",@"pid":@(controller),@"guardian_pid":@(getpid()),@"target_pid":@(pid),
        @"tap_id":@(identifier),@"disable_code":@(disabled),@"private_before":@(queriedBefore),
        @"tap_ids":ids,@"private_after":@(privateAfter),@"present":@(present),@"enabled":@(anyEnabled)});
    NSString *reason=@"disable-failed";
    if (!disabled && present && !anyEnabled) {
        reason=@"deadline";
        while (monotonic()<deadline) {
            if (stopping) { reason=@"signal"; break; }
            int result=poll(&lease,1,20);
            if (result>0) { reason=@"controller-disconnected"; break; }
            if (result<0 && !stopping) { reason=@"lease-error"; break; }
            BOOL valid=YES;
            for (uint32_t i=0;i<matched;i++) valid &= lookup(targets[i],expected,&enabled);
            if (!valid) { reason=@"target-changed"; break; }
        }
    }
    BOOL finalPresent=YES,finalEnabled=YES,anyRestore=NO;
    CGError restored=kCGErrorSuccess;
    NSMutableArray *restores=[NSMutableArray new];
    for (uint32_t i=0;i<attempted;i++) {
        BOOL current=NO;
        BOOL sameTarget=lookup(targets[i],expected,&current); anyRestore |= sameTarget;
        CGError code=sameTarget ? setEnabled(targets[i].tap.eventTapID,targets[i].tap.enabled):kCGErrorInvalidConnection;
        BOOL found=lookup(targets[i],expected,&current);
        finalPresent &= found; finalEnabled &= found && current;
        if (code) restored=code;
        [restores addObject:@{@"tap_id":@(targets[i].tap.eventTapID),@"attempted":@(sameTarget),
            @"code":@(code),@"present":@(found),@"enabled":@(current)}];
    }
    emit(@{@"kind":@"restored",@"pid":@(controller),@"guardian_pid":@(getpid()),@"target_pid":@(pid),
        @"tap_id":@(identifier),@"restore_code":@(restored),@"restore_attempted":@(anyRestore),@"taps":restores,
        @"reason":reason,@"present":@(finalPresent),@"enabled":@(finalEnabled)});
    dlclose(lib);
    // Target exit is safe teardown, but must not be reported as a restored tap.
    return (attempted==matched && anyRestore && !restored && finalPresent && finalEnabled) ? 0:3;
}
int main(int argc,char **argv) {
    @autoreleasepool {
        BOOL guardian=argc==4 && !strcmp(argv[1],"--guardian");
        if (!guardian && argc!=2) return 64;
        char *end=NULL; long number=strtol(argv[guardian ? 2:1],&end,10);
        if (!end || *end || number<=0 || number>INT_MAX) return 64;
        char executable[PATH_MAX]; uint32_t length=sizeof(executable);
        if (_NSGetExecutablePath(executable,&length)) return 65;
        char resolved[PATH_MAX]; if (!realpath(executable,resolved)) return 65;
        NSString *root=[[NSString stringWithUTF8String:resolved] stringByDeletingLastPathComponent];
        NSString *expected=[root stringByAppendingPathComponent:@"owner-mechanisms"];
        struct proc_bsdinfo info={0};
        if (!processIdentity((pid_t)number,expected,&info)) {
            // A second exact task-owned fixture, independently signed and granted
            // Accessibility through Settings; never a caller-supplied path.
            expected=[root stringByAppendingPathComponent:
                @"KeyCrash Permission Test Filter.app/Contents/MacOS/owner-mechanisms"];
            if (!processIdentity((pid_t)number,expected,&info)) return 65;
        }
        signal(SIGTERM,stopExperiment); signal(SIGINT,stopExperiment); signal(SIGHUP,stopExperiment);
        // A GUI/pipe reader can disappear with the controller. Logging must not
        // terminate the independent owner of the restore obligation.
        signal(SIGPIPE,SIG_IGN);
        if (guardian) return guard((pid_t)number,(pid_t)strtol(argv[3],NULL,10),expected);

        int lease[2]; if (pipe(lease)) return 71;
        posix_spawn_file_actions_t actions; posix_spawn_file_actions_init(&actions);
        posix_spawn_file_actions_adddup2(&actions,lease[0],STDIN_FILENO);
        posix_spawn_file_actions_addclose(&actions,lease[0]);
        posix_spawn_file_actions_addclose(&actions,lease[1]);
        char controller[32]; snprintf(controller,sizeof(controller),"%d",getpid());
        char *args[]={resolved,"--guardian",argv[1],controller,NULL};
        pid_t child=0;
        int error=posix_spawn(&child,resolved,&actions,NULL,args,environ);
        posix_spawn_file_actions_destroy(&actions); close(lease[0]);
        if (error) { close(lease[1]); return 71; }
        // No mutation here. Closing the sole writer releases the guardian on
        // SIGKILL/crash; its independent monotonic deadline bounds SIGSTOP.
        int status=0; double deadline=monotonic()+10;
        struct pollfd input={STDIN_FILENO,POLLIN,0};
        while (!stopping && monotonic()<deadline) {
            pid_t result=waitpid(child,&status,WNOHANG);
            if (result==child) { close(lease[1]); return WIFEXITED(status) ? WEXITSTATUS(status):70; }
            if (result<0 || poll(&input,1,20)>0) break;
        }
        close(lease[1]);
        while (waitpid(child,&status,0)<0 && errno==EINTR) {}
        return WIFEXITED(status) ? WEXITSTATUS(status):70;
    }
}

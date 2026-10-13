// Stable, development-only TCC host. The changing fixture lives beside the app.
// Launch through Finder/open; a direct SSH child has a different TCC identity.
#import <Foundation/Foundation.h>
#import <ApplicationServices/ApplicationServices.h>
#include <spawn.h>
#include <sys/wait.h>
extern char **environ;

int main(int argc, char **argv) {
    @autoreleasepool {
        if (argc == 2 && (strcmp(argv[1], "authorize") == 0 ||
                          strcmp(argv[1], "request-permission") == 0)) {
            if (strcmp(argv[1], "request-permission") == 0) {
                NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
                AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
                NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:30];
                while (!CGPreflightPostEventAccess() && deadline.timeIntervalSinceNow > 0)
                    [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.25]];
            }
            bool granted = CGPreflightPostEventAccess();
            printf("{\"kind\":\"permission\",\"post\":%s}\n", granted ? "true" : "false");
            return granted ? 0 : 7;
        }
        NSString *root = [[[NSBundle mainBundle] bundlePath] stringByDeletingLastPathComponent];
        NSString *binary = [root stringByAppendingPathComponent:@"owner-query"];
        char **args = calloc((size_t)argc + 1, sizeof(char *));
        if (!args) return 70;
        args[0] = (char *)binary.fileSystemRepresentation;
        for (int i = 1; i < argc; i++) args[i] = argv[i];
        pid_t pid = 0;
        int result = posix_spawn(&pid, args[0], NULL, NULL, args, environ);
        free(args);
        if (result) return 70;
        int status = 0;
        if (waitpid(pid, &status, 0) < 0) return 71;
        return WIFEXITED(status) ? WEXITSTATUS(status) : 72;
    }
}

/* macOS process supervisor: no model data, stdout or command logging.
 * Portable POSIX so the same lifecycle contract can be qualified on Linux. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
static volatile sig_atomic_t stopping = 0;
static void stop(int signal_number) { (void)signal_number; stopping = 1; }
int main(int argc, char **argv) {
    if (argc < 4 || argv[2][0] != '-' || argv[2][1] != '-' || argv[2][2]) return 2;
    char *end = NULL;
    long owner = strtol(argv[1], &end, 10);
    if (!end || *end || owner <= 1 || getppid() != (pid_t)owner) return 2;
    struct sigaction action = {0};
    action.sa_handler = stop;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGTERM, &action, NULL) || sigaction(SIGINT, &action, NULL) || sigaction(SIGHUP, &action, NULL)) return 2;
    pid_t child = fork();
    if (child < 0) return 2;
    if (child == 0) {
        setpgid(0, 0);
        action.sa_handler = SIG_DFL;
        sigaction(SIGTERM, &action, NULL); sigaction(SIGINT, &action, NULL); sigaction(SIGHUP, &action, NULL);
        execv(argv[3], argv + 3);
        _exit(127);
    }
    if (setpgid(child, child) < 0 && errno != EACCES && errno != ESRCH) stopping = 1;
    int status = 0;
    const struct timespec interval = {0, 100000000};
    for (;;) {
        pid_t result = waitpid(child, &status, WNOHANG);
        if (result == child) break;
        if (result < 0 && errno != EINTR) return 2;
        if (stopping || getppid() != (pid_t)owner) {
            kill(-child, SIGKILL);
            kill(child, SIGKILL);
            while (waitpid(child, &status, 0) < 0 && errno == EINTR) {}
            break;
        }
        nanosleep(&interval, NULL);
    }
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + (WIFSIGNALED(status) ? WTERMSIG(status) : 1);
}

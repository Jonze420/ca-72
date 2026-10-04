// A sampling profiler loaded with LD_PRELOAD: SIGPROF every SPROF_US us of CPU time
// records the interrupted instruction pointer; at exit the addresses (relative to the
// main executable's load address) go to SPROF_OUT, one per line, for a symbolizer. The
// samples in the first SPROF_SKIP_US us of CPU time (a program's setup) are left out.
#define _GNU_SOURCE
#include <signal.h>
#include <sys/time.h>
#include <ucontext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <link.h>
#include <time.h>
#define MAX 4000000
static unsigned long pcs[MAX];
static volatile unsigned long n;
static double skip_s;
static void handler(int sig, siginfo_t *si, void *uc_) {
    ucontext_t *uc = uc_;
    if (skip_s > 0) {
        struct timespec ts;
        clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &ts);
        if (ts.tv_sec + 1e-9 * ts.tv_nsec < skip_s) return;
    }
    unsigned long i = __atomic_fetch_add(&n, 1, __ATOMIC_RELAXED);
    if (i < MAX) pcs[i] = uc->uc_mcontext.gregs[REG_RIP];
}
static unsigned long base;
static int cb(struct dl_phdr_info *info, size_t size, void *data) {
    if (info->dlpi_name == NULL || info->dlpi_name[0] == 0) { base = info->dlpi_addr; return 1; }
    return 0;
}
__attribute__((constructor)) static void start(void) {
    struct sigaction sa; memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = handler; sa.sa_flags = SA_SIGINFO | SA_RESTART;
    sigaction(SIGPROF, &sa, NULL);
    const char *us = getenv("SPROF_US");
    long u = us ? atol(us) : 200;
    const char *sk = getenv("SPROF_SKIP_US");
    skip_s = sk ? 1e-6 * atol(sk) : 0;
    struct itimerval it = { { 0, u }, { 0, u } };
    setitimer(ITIMER_PROF, &it, NULL);
}
__attribute__((destructor)) static void stop(void) {
    struct itimerval it = { { 0, 0 }, { 0, 0 } };
    setitimer(ITIMER_PROF, &it, NULL);
    dl_iterate_phdr(cb, NULL);
    const char *out = getenv("SPROF_OUT");
    FILE *f = fopen(out ? out : "sprof.txt", "w");
    unsigned long m = n < MAX ? n : MAX;
    for (unsigned long i = 0; i < m; i++) fprintf(f, "%lx\n", pcs[i] - base);
    fclose(f);
}

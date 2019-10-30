#include <stdint.h>

#ifdef __x86_64__

void cpuid(unsigned int op, unsigned int *a, unsigned int *b, unsigned int *c, unsigned int *d)
{
  asm volatile (".byte 0x53\n\tcpuid\n\t"
                "movl %%ebx, %%esi\n\t.byte 0x5b"
                : "=a" (*a),  "=S" (*b),  "=c" (*c),  "=d" (*d)
                : "a" (op));
}

uint64_t rdpmc(uint32_t counter) {
  uint32_t low, high;
  asm volatile("rdpmc" : "=a" (low), "=d" (high) : "c" (counter));
  return low | ((uint64_t)high) << 32;
}

#endif

#ifdef __powerpc64__

#define STRINGIFY_1(X...) #X
#define STRINGIFY(X...) stringify_1(X)
#define MFSPR(rn) ({unsigned long rval; asm volatile("mfspr %0," STRINGIFY(rn) : "=r" (rval)); rval; })

unsigned long mfspr_pvr() {
  unsigned long pvr = MFSPR(0x11F);
  return pvr;
}

#endif
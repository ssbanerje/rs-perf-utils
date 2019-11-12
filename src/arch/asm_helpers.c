#include <stdint.h>

#ifdef __x86_64__

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
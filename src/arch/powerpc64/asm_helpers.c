#include <stdint.h>

uint64_t mfspr_pvr() {
  uint64_t pvr;
  asm volatile("mfspr %0," "0x11F" : "=r" (pvr));
  return pvr;
}

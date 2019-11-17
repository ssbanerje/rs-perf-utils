#include <stdint.h>

uint64_t rdpmc(uint32_t counter) {
  uint32_t low, high;
  asm volatile("rdpmc" : "=a" (low), "=d" (high) : "c" (counter));
  return low | ((uint64_t)high) << 32;
}

/* The C side of tests/interop: calls NEXUL, which calls back into C. The declarations
   come from the header that nxc writes (`--header nxlib.h`). */
#include <stdio.h>

#include "nxlib.h"

static int32_t twice(int32_t x) { return 2 * x; }

static int32_t square(int32_t x) { return x * x; }

int main(void) {
    printf("add %d\n", nx_add(2, 40));
    printf("apply %d\n", nx_apply(square, twice, 7));
    printf("words %d\n", (int)nx_words((uint8_t *)"  le compilateur  NEXUL appelle C "));
    printf("choose %d %d\n", nx_choose(true, 1, 2), nx_choose(false, 1, 2));
    Point a = {2, 10}, b = {6, -4};
    Point m = nx_middle(a, b);
    printf("middle %d %d\n", m.x, m.y);
    fflush(stdout);
    for (int i = 0; i < 3; i++) {
        nx_hello(i);
    }
    printf("back in C\n");
    return 0;
}

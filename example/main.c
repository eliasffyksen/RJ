#include <stdio.h>

void test(int *, int *, int *, int *, int, int);

int main() {
  int a, b, c, d, e, f;
  a = 5;
  b = 6;

  test(&c, &d, &e, &f, a, b);

  printf("%d %d %d %d %d %d!\n", a, b, c, d, e, f);
}

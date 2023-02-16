#include <stdio.h>

void test(int *, int *, int *, int);

int random() {
  return 666;
}

int main() {
  int a, b = 9, c;

  test(&a, &b, &c, b);

  printf("%d %d %d!\n", a, b, c);
}

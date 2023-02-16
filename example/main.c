#include <stdio.h>

void test(int *);

int random() {
  return 666;
}

int main() {
  int a;

  test(&a);

  printf("%d!\n", a);
}

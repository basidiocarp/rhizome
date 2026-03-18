#include <stdio.h>
#include <stdlib.h>

#define MAX_SIZE 1024

#define SQUARE(x) ((x) * (x))

typedef unsigned long usize;

struct Config {
    char *name;
    int value;
};

enum Status {
    STATUS_OK,
    STATUS_ERROR,
    STATUS_PENDING
};

void process(struct Config *cfg) {
    printf("Processing: %s\n", cfg->name);
}

int calculate(int a, int b) {
    return a + b;
}

typedef struct {
    int x;
    int y;
} Point;

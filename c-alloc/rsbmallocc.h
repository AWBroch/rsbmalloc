#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

void *malloc(size_t size);

void free(void *ptr);

void *calloc(size_t count, size_t size);

void *realloc(void *ptr, size_t size);

void *aligned_alloc(size_t alignment, size_t size);

void *valloc(size_t size);

void *pvalloc(size_t size);

void *memalign(size_t alignment, size_t size);

int posix_memalign(void **memptr, size_t alignment, size_t size);

void *rsbmalloc(size_t size);

void rsbfree(void *ptr);

void *rsbcalloc(size_t count, size_t size);

void *rsbrealloc(void *ptr, size_t size);

void *rsbaligned_alloc(size_t alignment, size_t size);

void *rsbvalloc(size_t size);

void *rsbpvalloc(size_t size);

void *rsbmemalign(size_t alignment, size_t size);

int rsbposix_memalign(void **memptr, size_t alignment, size_t size);

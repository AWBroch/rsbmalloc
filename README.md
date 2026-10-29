# rsbmalloc

A binned allocator for Rust. It’s quite simple, but reasonably fast single and multi-threaded. Single-threaded, it generally similar to the built-in allocator, sometimes faster, but sometimes with higher memory usage. Multi-threaded, it ranges from similar speed to quite a bit slower. It’s pure Rust, so it should work smoothly on any platform that provides standard `mmap` and `munmap` functions, and also Windows (though Windows support isn’t tested).

Multi-threaded support uses shared caches selected by a hash of the thread ID. On successful initialization, four times the online CPU count reported by the OS are created. Each cache uses locks because multiple threads can select the same cache. CPU discovery uses OS calls without heap allocation; cache sizing does not account for container CPU quotas.

`rsbmalloc` is entirely a binned allocator, with bins ranging from 4 bytes to 64 KiB. If an allocation is larger than 64 KiB, it gets counted as a large allocation and goes straight to `mmap` and `munmap`. So, when freed in Rust, it gets `munmap`-ed. Bins, however, obtain 64 KiB chunks as necessary and are never released back to the OS. Freed slots just act as a linked list that can be reused by the same thread (or another thread that scores the same thread cache).

It implements the `GlobalAlloc` trait, and comes with a single-threaded `no_std` version. The `no_std` version still requires a libc with `mmap` and `munmap` or Windows, but it doesn’t depend on the Rust standard library. Note that the `no_std` version is still thead-safe, it just doesn’t use the thread-local caches, so it’s a _lot_ slower because it relies on spinlocks when operating multi-threaded. On the other hand, it uses less memory and would be a similar speed if there’s no lock contention. Once the `allocator-api` is stable, it should be a fairly easy port to that.

`rsbmalloc` also exposes the page-only allocator it uses under the hood. `PageAllocator` accepts alignments up to the OS page size and returns null for larger alignments, invalid page-size discovery, or allocation failure. `RSBMalloc` accepts alignments up to 4 KiB (subject to the OS page alignment being sufficient for its bins).

Allocation failures return null without dereferencing the failed allocation. If thread-cache initialization fails, allocations use embedded synchronized fallback bins and retry initialization on a later call. Reallocation preserves the original allocation and contents on failure. Page reallocations that change the rounded mapping size allocate a replacement, copy the preserved bytes, and release the old mapping; this can require more copying and temporary memory than in-place resizing. OS deallocation errors are still silently ignored.

A [Broch Web Solutions](https://www.brochweb.com/) project.

Check out [the blog post](https://www.brochweb.com/blog/post/how-to-create-a-custom-memory-allocator-in-rust/) for more info.

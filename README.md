# binned-alloc-rs

A binned allocator for Rust. It’s quite simple, but reasonably fast single and multi-threaded. Single-threaded, it generally similar to the built-in allocator, sometimes faster, but sometimes with higher memory usage. Multi-threaded, it ranges from similar speed to quite a bit slower. It’s pure Rust, so it should work smoothly on any platform that provides standard `mmap` and `munmap` functions, and also Windows (though Windows support isn’t tested).

Relies exclusively on thread-local caches for multi-threaded support. 4 times the number of cpus are created on the first allocation and no more are created after that, so each ‘thread-local’ cache is fully thread-safe in case it is reused between threads.

`binned-alloc-rs` is entirely a binned allocator, with bins ranging from 4 bytes to 16 KiB (some ARM pages sizes are 16 KiB). If an allocation is larger than 16 KiB, it gets counted as a large allocation and goes straight to `mmap` and `munmap`. So, when freed in Rust, it gets `munmap`-ed. Bins, however, are allocated a page at a time as necessary and are never released back to the OS. Freed slots just act as a linked list that can be reused by the same thread (or another thread that scores the same thread cache).

It implements the `GlobalAllocator` trait, and comes with a single-threaded `no_std` version. The `no_std` version still requires a libc with `mmap` and `munmap` or Windows, but it doesn’t depend on the Rust standard library. Once the `allocator-api` is stable, it should be a fairly easy port to that.

`binned-alloc-rs` also exposes the page-only allocator it uses under the hood.

A [Broch Web Solutions](https://www.brochweb.com/) project.
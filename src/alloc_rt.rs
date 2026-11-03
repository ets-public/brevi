// ---------- memory ----------
// Small blocks (up to 256 bytes: most strings, list and dict nodes, boxed values) come from per-thread free
// lists of 16-byte size classes, carved from 256KB chunks; bigger ones go to the system allocator. Python
// programs make and drop many small objects, where this is several times quicker than malloc and free.
// A block freed on another thread joins that thread's lists; chunks are never handed back.
pub struct AxAlloc;
const AX_SMALL: usize = 256;
const AX_CHUNK: usize = 256 << 10;
struct AxPool {
    free: [std::cell::Cell<*mut u8>; AX_SMALL / 16 + 1],
    next: std::cell::Cell<*mut u8>,
    end: std::cell::Cell<*mut u8>,
}
thread_local! {
    static AX_POOL: AxPool = const { AxPool {
        free: [const { std::cell::Cell::new(std::ptr::null_mut()) }; AX_SMALL / 16 + 1],
        next: std::cell::Cell::new(std::ptr::null_mut()),
        end: std::cell::Cell::new(std::ptr::null_mut()),
    } };
}
#[inline(always)]
fn ax_small(l: &std::alloc::Layout) -> bool {
    l.size() <= AX_SMALL && l.align() <= 16 && l.size() > 0
}
unsafe impl std::alloc::GlobalAlloc for AxAlloc {
    #[inline]
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if !ax_small(&l) {
            return std::alloc::System.alloc(l);
        }
        let c = (l.size() + 15) >> 4;
        let p = AX_POOL.try_with(|pool| {
            let head = pool.free[c].get();
            if !head.is_null() {
                pool.free[c].set(*(head as *mut *mut u8));
                return head;
            }
            let sz = c << 4;
            let mut n = pool.next.get();
            if n.is_null() || (pool.end.get() as usize) - (n as usize) < sz {
                let chunk = std::alloc::System.alloc(std::alloc::Layout::from_size_align_unchecked(AX_CHUNK, 16));
                if chunk.is_null() {
                    return chunk;
                }
                n = chunk;
                pool.end.set(chunk.add(AX_CHUNK));
            }
            pool.next.set(n.add(sz));
            n
        });
        match p {
            Ok(p) => p,
            // During thread teardown: a plain block of the class size, which free() can still take.
            Err(_) => std::alloc::System.alloc(std::alloc::Layout::from_size_align_unchecked(c << 4, 16)),
        }
    }
    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        if !ax_small(&l) {
            return std::alloc::System.dealloc(p, l);
        }
        let c = (l.size() + 15) >> 4;
        let _ = AX_POOL.try_with(|pool| {
            *(p as *mut *mut u8) = pool.free[c].get();
            pool.free[c].set(p);
        });
    }
    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        if !ax_small(&l) && new > AX_SMALL {
            return std::alloc::System.realloc(p, l, new);
        }
        if ax_small(&l) && new <= AX_SMALL && new > 0 && (new + 15) >> 4 == (l.size() + 15) >> 4 {
            return p;
        }
        let nl = std::alloc::Layout::from_size_align_unchecked(new, l.align());
        let q = self.alloc(nl);
        if !q.is_null() {
            std::ptr::copy_nonoverlapping(p, q, l.size().min(new));
            self.dealloc(p, l);
        }
        q
    }
}
#[global_allocator]
static AX_GLOBAL: AxAlloc = AxAlloc;

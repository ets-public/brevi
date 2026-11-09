// ---------- memory ----------
// Small blocks (up to 256 bytes: most strings, list and dict nodes, boxed values) come from 16KB pages, each
// holding blocks of one 16-byte size class; bigger ones go to the system allocator. Python programs make and
// drop many small objects, where this is several times quicker than malloc and free.
// As in CPython's pymalloc, a freed block goes back to its own page, and a thread allocates from one page of a
// class until that page is full: objects made one after another sit together in memory even after many
// rounds of making and dropping them (one free list per class hands blocks back in the order they were freed,
// which scatters a program's objects over the whole heap). A page belongs to the thread that carved it; a
// block freed on another thread joins that thread's own list for the class. Pages are never handed back.
pub struct AxAlloc;
// The last long text found to be all ASCII (its address, length and capacity), so indexing it again need not
// scan it; freeing or moving that block forgets it (see ax_ascii in the prelude).
pub static AX_ASCII: [std::sync::atomic::AtomicUsize; 3] = [const { std::sync::atomic::AtomicUsize::new(0) }; 3];
#[inline(always)]
fn ax_forget_ascii(p: *mut u8) {
    if AX_ASCII[0].load(std::sync::atomic::Ordering::Relaxed) == p as usize {
        AX_ASCII[0].store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
const AX_SMALL: usize = 256;
const AX_CLASSES: usize = AX_SMALL / 16 + 1;
const AX_PAGE: usize = 16 << 10;
const AX_CHUNK: usize = 256 << 10;
// at the start of each page; blocks start at AX_HDR
#[repr(C)]
struct AxPage {
    free: *mut u8,
    next: *mut u8,
    end: *mut u8,
    // the owning thread's pool (its address), 0 for the shared pool used while a thread is being torn down
    owner: usize,
    // the next page of this class with free blocks, while in_list
    link: *mut AxPage,
    in_list: bool,
}
const AX_HDR: usize = (std::mem::size_of::<AxPage>() + 15) & !15;
struct AxPool {
    // per class: the page being allocated from, other pages with free blocks, blocks of other threads' pages
    cur: [std::cell::Cell<*mut AxPage>; AX_CLASSES],
    partial: [std::cell::Cell<*mut AxPage>; AX_CLASSES],
    loose: [std::cell::Cell<*mut u8>; AX_CLASSES],
    // pages not yet given a class, in the newest chunk
    pnext: std::cell::Cell<*mut u8>,
    pend: std::cell::Cell<*mut u8>,
}
impl AxPool {
    const fn new() -> AxPool {
        AxPool {
            cur: [const { std::cell::Cell::new(std::ptr::null_mut()) }; AX_CLASSES],
            partial: [const { std::cell::Cell::new(std::ptr::null_mut()) }; AX_CLASSES],
            loose: [const { std::cell::Cell::new(std::ptr::null_mut()) }; AX_CLASSES],
            pnext: std::cell::Cell::new(std::ptr::null_mut()),
            pend: std::cell::Cell::new(std::ptr::null_mut()),
        }
    }
    #[inline(always)]
    unsafe fn alloc(&self, c: usize) -> *mut u8 {
        let p = self.cur[c].get();
        if !p.is_null() {
            let b = (*p).free;
            if !b.is_null() {
                (*p).free = *(b as *mut *mut u8);
                return b;
            }
            let n = (*p).next;
            if n < (*p).end {
                (*p).next = n.add(c << 4);
                return n;
            }
        }
        self.alloc_slow(c)
    }
    #[inline(never)]
    unsafe fn alloc_slow(&self, c: usize) -> *mut u8 {
        // blocks of other threads' pages freed here
        let l = self.loose[c].get();
        if !l.is_null() {
            self.loose[c].set(*(l as *mut *mut u8));
            return l;
        }
        // a page of this class with free blocks, else a new page
        let mut p = self.partial[c].get();
        if !p.is_null() {
            self.partial[c].set((*p).link);
            (*p).in_list = false;
        } else {
            let mut n = self.pnext.get();
            if n.is_null() || n >= self.pend.get() {
                let chunk = std::alloc::GlobalAlloc::alloc(&std::alloc::System, std::alloc::Layout::from_size_align_unchecked(AX_CHUNK, AX_PAGE));
                if chunk.is_null() {
                    return chunk;
                }
                n = chunk;
                self.pend.set(chunk.add(AX_CHUNK));
            }
            self.pnext.set(n.add(AX_PAGE));
            p = n as *mut AxPage;
            let sz = c << 4;
            let first = n.add(AX_HDR);
            std::ptr::write(p, AxPage { free: std::ptr::null_mut(), next: first, end: n.add(AX_HDR + (AX_PAGE - AX_HDR) / sz * sz), owner: self as *const AxPool as usize, link: std::ptr::null_mut(), in_list: false });
        }
        self.cur[c].set(p);
        self.alloc(c)
    }
    #[inline(always)]
    unsafe fn free(&self, b: *mut u8, c: usize) {
        let p = (b as usize & !(AX_PAGE - 1)) as *mut AxPage;
        if (*p).owner != self as *const AxPool as usize {
            *(b as *mut *mut u8) = self.loose[c].get();
            self.loose[c].set(b);
            return;
        }
        *(b as *mut *mut u8) = (*p).free;
        (*p).free = b;
        if !(*p).in_list && p != self.cur[c].get() {
            (*p).in_list = true;
            (*p).link = self.partial[c].get();
            self.partial[c].set(p);
        }
    }
}
thread_local! {
    static AX_POOL: AxPool = const { AxPool::new() };
}
// while a thread is torn down (its pool gone), small blocks come from this shared pool, under a lock
struct AxShared(std::sync::atomic::AtomicBool, AxPool);
unsafe impl Sync for AxShared {}
static AX_SHARED: AxShared = AxShared(std::sync::atomic::AtomicBool::new(false), AxPool::new());
fn ax_shared<T>(f: impl FnOnce(&AxPool) -> T) -> T {
    while AX_SHARED.0.compare_exchange_weak(false, true, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
    let r = f(&AX_SHARED.1);
    AX_SHARED.0.store(false, std::sync::atomic::Ordering::Release);
    r
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
        match AX_POOL.try_with(|pool| pool.alloc(c)) {
            Ok(p) => p,
            Err(_) => ax_shared(|pool| pool.alloc(c)),
        }
    }
    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        if !ax_small(&l) {
            ax_forget_ascii(p);
            return std::alloc::System.dealloc(p, l);
        }
        let c = (l.size() + 15) >> 4;
        if AX_POOL.try_with(|pool| pool.free(p, c)).is_err() {
            ax_shared(|pool| pool.free(p, c));
        }
    }
    #[inline]
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        if !ax_small(&l) {
            ax_forget_ascii(p);
        }
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

// ---------- numpy's default (quicksort) argsort order, exactly ----------
// numpy 2.x dispatches ascending argsort for int32/int64/float32/float64 to x86-simd-sort
// (AVX-512 on X86_V4 CPUs, AVX2 on X86_V3 CPUs) and falls back to its own introsort
// (numpy/_core/src/npysort/quicksort.hpp) elsewhere. The unstable order of equal values is
// the algorithm's, so all three are reproduced here: the x86-simd-sort argsort is emulated
// lane-by-lane in scalar Rust (the SIMD only runs the same network/partition in parallel),
// and the introsort fallback is a direct port. On x86_64 the path is chosen at run time from
// the CPU features, using numpy's exact X86_V4 / X86_V3 feature groups.

// K is one of i64, i32, f64, f32. The x86-simd-sort path is only called without NaNs (numpy
// hands those arrays to std::sort, whose tie order is not reproduced; the caller refuses).
trait Kops {
    type K: Copy + PartialEq;
    fn lt(a: Self::K, b: Self::K) -> bool;
    // numpy's quicksort comparator (NaN last), used by the introsort fallback
    fn intro_lt(a: Self::K, b: Self::K) -> bool;
    fn min2(a: Self::K, b: Self::K) -> Self::K;
    fn max2(a: Self::K, b: Self::K) -> Self::K;
    fn type_max() -> Self::K;
    fn type_min() -> Self::K;
    fn any_nan(_v: &[Self::K]) -> bool {
        false
    }
}
macro_rules! int_ops {
    ($t:ty) => {
        impl Kops for $t {
            type K = $t;
            fn lt(a: $t, b: $t) -> bool {
                a < b
            }
            fn intro_lt(a: $t, b: $t) -> bool {
                a < b
            }
            fn min2(a: $t, b: $t) -> $t {
                if b < a {
                    b
                } else {
                    a
                }
            }
            fn max2(a: $t, b: $t) -> $t {
                if a < b {
                    b
                } else {
                    a
                }
            }
            fn type_max() -> $t {
                <$t>::MAX
            }
            fn type_min() -> $t {
                <$t>::MIN
            }
        }
    };
}
int_ops!(i64);
int_ops!(i32);
macro_rules! float_ops {
    ($t:ty) => {
        impl Kops for $t {
            type K = $t;
            fn lt(a: $t, b: $t) -> bool {
                a < b
            }
            fn intro_lt(a: $t, b: $t) -> bool {
                a < b || (b != b && a == a)
            }
            fn min2(a: $t, b: $t) -> $t {
                if b < a {
                    b
                } else {
                    a
                }
            }
            fn max2(a: $t, b: $t) -> $t {
                if a < b {
                    b
                } else {
                    a
                }
            }
            fn type_max() -> $t {
                <$t>::INFINITY
            }
            fn type_min() -> $t {
                -<$t>::INFINITY
            }
            fn any_nan(v: &[$t]) -> bool {
                v.iter().any(|x| x.is_nan())
            }
        }
    };
}
float_ops!(f64);
float_ops!(f32);

fn msb(n: usize) -> i32 {
    (usize::BITS - 1 - n.leading_zeros()) as i32
}

// ---------- numpy's own introsort argsort (aquicksort_ + aheapsort_) ----------

fn intro_heap<K: Kops>(v: &[K::K], out: &mut [usize], n: usize) {
    // npy_aheapsort: 1-based heap over out[0..n] (a[i] == out[i-1])
    let mut l = n >> 1;
    while l > 0 {
        let tmp = out[l - 1];
        let mut i = l;
        let mut j = l << 1;
        while j <= n {
            if j < n && K::intro_lt(v[out[j - 1]], v[out[j]]) {
                j += 1;
            }
            if K::intro_lt(v[tmp], v[out[j - 1]]) {
                out[i - 1] = out[j - 1];
                i = j;
                j += j;
            } else {
                break;
            }
        }
        out[i - 1] = tmp;
        l -= 1;
    }
    let mut m = n;
    while m > 1 {
        let tmp = out[m - 1];
        out[m - 1] = out[0];
        m -= 1;
        let mut i = 1;
        let mut j = 2;
        while j <= m {
            if j < m && K::intro_lt(v[out[j - 1]], v[out[j]]) {
                j += 1;
            }
            if K::intro_lt(v[tmp], v[out[j - 1]]) {
                out[i - 1] = out[j - 1];
                i = j;
                j += j;
            } else {
                break;
            }
        }
        out[i - 1] = tmp;
    }
}

fn intro_argsort<K: Kops>(v: &[K::K], out: &mut [usize]) {
    let n = v.len();
    if n <= 1 {
        return;
    }
    let mut stack: Vec<(usize, usize, i32)> = Vec::new();
    let mut pl = 0usize;
    let mut pr = n - 1;
    let mut cdepth = msb(n) * 2;
    loop {
        if cdepth < 0 {
            intro_heap::<K>(v, &mut out[pl..=pr], pr - pl + 1);
            match stack.pop() {
                Some((l, r, d)) => {
                    pl = l;
                    pr = r;
                    cdepth = d;
                    continue;
                }
                None => return,
            }
        }
        while pr - pl > 15 {
            let pm = pl + ((pr - pl) >> 1);
            if K::intro_lt(v[out[pm]], v[out[pl]]) {
                out.swap(pm, pl);
            }
            if K::intro_lt(v[out[pr]], v[out[pm]]) {
                out.swap(pr, pm);
            }
            if K::intro_lt(v[out[pm]], v[out[pl]]) {
                out.swap(pm, pl);
            }
            let vp = v[out[pm]];
            let mut pi = pl;
            let mut pj = pr - 1;
            out.swap(pm, pj);
            loop {
                pi += 1;
                while K::intro_lt(v[out[pi]], vp) {
                    pi += 1;
                }
                pj -= 1;
                while K::intro_lt(vp, v[out[pj]]) {
                    pj -= 1;
                }
                if pi >= pj {
                    break;
                }
                out.swap(pi, pj);
            }
            out.swap(pi, pr - 1);
            if pi - pl < pr - pi {
                stack.push((pi + 1, pr, cdepth - 1));
                pr = pi - 1;
            } else {
                stack.push((pl, pi - 1, cdepth - 1));
                pl = pi + 1;
            }
            cdepth -= 1;
        }
        for pi in (pl + 1)..=pr {
            let vi = out[pi];
            let vp = v[vi];
            let mut pj = pi;
            while pj > pl && K::intro_lt(vp, v[out[pj - 1]]) {
                out[pj] = out[pj - 1];
                pj -= 1;
            }
            out[pj] = vi;
        }
        match stack.pop() {
            Some((l, r, d)) => {
                pl = l;
                pr = r;
                cdepth = d;
            }
            None => return,
        }
    }
}

// ---------- x86-simd-sort argsort, emulated lane by lane ----------

fn mask_bits<const L: usize>(bits: u64) -> [bool; L] {
    let mut m = [false; L];
    for l in 0..L {
        m[l] = (bits >> l) & 1 == 1;
    }
    m
}

fn swap_n<const SCALE: usize, const L: usize, K: Kops>(x: &mut [K::K; L]) {
    let mut base = 0;
    while base < L {
        for j in 0..SCALE / 2 {
            x.swap(base + j, base + SCALE / 2 + j);
        }
        base += SCALE;
    }
}
fn reverse_n<const SCALE: usize, const L: usize, K: Kops>(x: &mut [K::K; L]) {
    let mut base = 0;
    while base < L {
        let (mut lo, mut hi) = (base, base + SCALE - 1);
        while lo < hi {
            x.swap(lo, hi);
            lo += 1;
            hi -= 1;
        }
        base += SCALE;
    }
}
fn swap_n_u<const SCALE: usize, const L: usize>(x: &mut [usize; L]) {
    let mut base = 0;
    while base < L {
        for j in 0..SCALE / 2 {
            x.swap(base + j, base + SCALE / 2 + j);
        }
        base += SCALE;
    }
}
fn reverse_n_u<const SCALE: usize, const L: usize>(x: &mut [usize; L]) {
    let mut base = 0;
    while base < L {
        let (mut lo, mut hi) = (base, base + SCALE - 1);
        while lo < hi {
            x.swap(lo, hi);
            lo += 1;
            hi -= 1;
        }
        base += SCALE;
    }
}

// xss-common-qsort.h cmp_merge: mask lanes take max(in2,in1), the rest min(in2,in1)
fn cmp_merge_single<const L: usize, K: Kops>(in1: &[K::K; L], in2: &[K::K; L], mask: &[bool; L]) -> [K::K; L] {
    let mut out = [K::type_max(); L];
    for l in 0..L {
        let mn = K::min2(in2[l], in1[l]);
        let mx = K::max2(in2[l], in1[l]);
        out[l] = if mask[l] { mx } else { mn };
    }
    out
}

// key/index cmp_merge: when the chosen key came from in1 keep index1, else index2
fn cmp_merge_pair<const L: usize, K: Kops>(
    key_in1: &[K::K; L],
    key_in2: &[K::K; L],
    idx_in1: &[usize; L],
    idx_in2: &[usize; L],
    mask: &[bool; L],
) -> ([K::K; L], [usize; L]) {
    let keys = cmp_merge_single::<L, K>(key_in1, key_in2, mask);
    let mut idx = [0usize; L];
    for l in 0..L {
        idx[l] = if keys[l] == key_in1[l] { idx_in1[l] } else { idx_in2[l] };
    }
    (keys, idx)
}

// sort_reg_*lanes<keyType,indexType> from xss-reg-networks.hpp
fn sort_reg_pair<const L: usize, K: Kops>(key: &mut [K::K; L], idx: &mut [usize; L]) {
    if L == 8 {
        let m_aa = mask_bits::<L>(0xAA);
        let m_cc = mask_bits::<L>(0xCC);
        let m_f0 = mask_bits::<L>(0xF0);
        macro_rules! step {
            ($kf:expr, $if:expr, $mask:expr) => {{
                let mut k2 = *key;
                let mut i2 = *idx;
                $kf(&mut k2);
                $if(&mut i2);
                let (nk, ni) = cmp_merge_pair::<L, K>(key, &k2, idx, &i2, $mask);
                *key = nk;
                *idx = ni;
            }};
        }
        step!(reverse_n::<2, L, K>, reverse_n_u::<2, L>, &m_aa);
        step!(reverse_n::<4, L, K>, reverse_n_u::<4, L>, &m_cc);
        step!(swap_n::<2, L, K>, swap_n_u::<2, L>, &m_aa);
        step!(reverse_n::<8, L, K>, reverse_n_u::<8, L>, &m_f0);
        step!(swap_n::<4, L, K>, swap_n_u::<4, L>, &m_cc);
        step!(swap_n::<2, L, K>, swap_n_u::<2, L>, &m_aa);
    } else {
        let m_a = mask_bits::<L>(0xA);
        let m_c = mask_bits::<L>(0xC);
        macro_rules! step4 {
            ($kf:expr, $if:expr, $mask:expr) => {{
                let mut k2 = *key;
                let mut i2 = *idx;
                $kf(&mut k2);
                $if(&mut i2);
                let (nk, ni) = cmp_merge_pair::<L, K>(key, &k2, idx, &i2, $mask);
                *key = nk;
                *idx = ni;
            }};
        }
        step4!(reverse_n::<2, L, K>, reverse_n_u::<2, L>, &m_a);
        step4!(reverse_n::<4, L, K>, reverse_n_u::<4, L>, &m_c);
        step4!(swap_n::<2, L, K>, swap_n_u::<2, L>, &m_a);
    }
}

// single-register value sort (used for the pivot's median), no indices
fn sort_reg<const L: usize, K: Kops>(key: &mut [K::K; L]) {
    if L == 8 {
        let m_aa = mask_bits::<L>(0xAA);
        let m_cc = mask_bits::<L>(0xCC);
        let m_f0 = mask_bits::<L>(0xF0);
        macro_rules! step {
            ($f:expr, $mask:expr) => {{
                let mut k2 = *key;
                $f(&mut k2);
                *key = cmp_merge_single::<L, K>(key, &k2, $mask);
            }};
        }
        step!(reverse_n::<2, L, K>, &m_aa);
        step!(reverse_n::<4, L, K>, &m_cc);
        step!(swap_n::<2, L, K>, &m_aa);
        step!(reverse_n::<8, L, K>, &m_f0);
        step!(swap_n::<4, L, K>, &m_cc);
        step!(swap_n::<2, L, K>, &m_aa);
    } else {
        let m_a = mask_bits::<L>(0xA);
        let m_c = mask_bits::<L>(0xC);
        macro_rules! step4 {
            ($f:expr, $mask:expr) => {{
                let mut k2 = *key;
                $f(&mut k2);
                *key = cmp_merge_single::<L, K>(key, &k2, $mask);
            }};
        }
        step4!(reverse_n::<2, L, K>, &m_a);
        step4!(reverse_n::<4, L, K>, &m_c);
        step4!(swap_n::<2, L, K>, &m_a);
    }
}

// bitonic_merge_reg_*lanes<keyType,indexType>: half cleaner for an already bitonic reg
fn bitonic_merge_reg<const L: usize, K: Kops>(key: &mut [K::K; L], idx: &mut [usize; L]) {
    if L == 8 {
        let m_aa = mask_bits::<L>(0xAA);
        let m_cc = mask_bits::<L>(0xCC);
        let m_f0 = mask_bits::<L>(0xF0);
        macro_rules! step {
            ($kf:expr, $if:expr, $mask:expr) => {{
                let mut k2 = *key;
                let mut i2 = *idx;
                $kf(&mut k2);
                $if(&mut i2);
                let (nk, ni) = cmp_merge_pair::<L, K>(key, &k2, idx, &i2, $mask);
                *key = nk;
                *idx = ni;
            }};
        }
        step!(swap_n::<8, L, K>, swap_n_u::<8, L>, &m_f0);
        step!(swap_n::<4, L, K>, swap_n_u::<4, L>, &m_cc);
        step!(swap_n::<2, L, K>, swap_n_u::<2, L>, &m_aa);
    } else {
        let m_a = mask_bits::<L>(0xA);
        let m_c = mask_bits::<L>(0xC);
        macro_rules! step4 {
            ($kf:expr, $if:expr, $mask:expr) => {{
                let mut k2 = *key;
                let mut i2 = *idx;
                $kf(&mut k2);
                $if(&mut i2);
                let (nk, ni) = cmp_merge_pair::<L, K>(key, &k2, idx, &i2, $mask);
                *key = nk;
                *idx = ni;
            }};
        }
        step4!(swap_n::<4, L, K>, swap_n_u::<4, L>, &m_c);
        step4!(swap_n::<2, L, K>, swap_n_u::<2, L>, &m_a);
    }
}

fn coex_pair<const L: usize, K: Kops>(k1: &mut [K::K; L], k2: &mut [K::K; L], i1: &mut [usize; L], i2: &mut [usize; L]) {
    let k1_orig = *k1;
    for l in 0..L {
        let nk1 = K::min2(k1[l], k2[l]);
        let nk2 = K::max2(k1_orig[l], k2[l]);
        let eq = nk1 == k1_orig[l];
        let (a, b) = (i1[l], i2[l]);
        i1[l] = if eq { a } else { b };
        i2[l] = if eq { b } else { a };
        k1[l] = nk1;
        k2[l] = nk2;
    }
}

fn coex_at<const L: usize, K: Kops>(keys: &mut [[K::K; L]], idxs: &mut [[usize; L]], a: usize, b: usize) {
    let (lo, hi) = keys.split_at_mut(b);
    let (loi, hii) = idxs.split_at_mut(b);
    coex_pair::<L, K>(&mut lo[a], &mut hi[0], &mut loi[a], &mut hii[0]);
}

fn bitonic_merge_n<const L: usize, K: Kops>(keys: &mut [[K::K; L]], idxs: &mut [[usize; L]]) {
    let numper = keys.len();
    if numper == 2 {
        let (lo, hi) = keys.split_at_mut(1);
        let (loi, hii) = idxs.split_at_mut(1);
        hi[0].reverse();
        hii[0].reverse();
        coex_pair::<L, K>(&mut lo[0], &mut hi[0], &mut loi[0], &mut hii[0]);
        hi[0].reverse();
        hii[0].reverse();
    } else {
        for i in 0..numper / 2 {
            let j = numper - 1 - i;
            keys[j].reverse();
            idxs[j].reverse();
            coex_at::<L, K>(keys, idxs, i, j);
            keys[j].reverse();
            idxs[j].reverse();
        }
    }
    let mut num = numper / 2;
    while num >= 2 {
        let mut j = 0;
        while j < numper {
            for i in 0..num / 2 {
                coex_at::<L, K>(keys, idxs, j + i, j + i + num / 2);
            }
            j += num;
        }
        num /= 2;
    }
    for i in 0..numper {
        bitonic_merge_reg::<L, K>(&mut keys[i], &mut idxs[i]);
    }
}

fn bitonic_fullmerge<const L: usize, K: Kops>(keys: &mut Vec<[K::K; L]>, idxs: &mut Vec<[usize; L]>) {
    let nv = keys.len();
    let mut numper = 2;
    while numper <= nv {
        let mut start = 0;
        while start < nv {
            let split = start + numper;
            let (head, _) = keys.split_at_mut(split);
            let (ihead, _) = idxs.split_at_mut(split);
            bitonic_merge_n::<L, K>(&mut head[start..], &mut ihead[start..]);
            start += numper;
        }
        numper *= 2;
    }
}

// sort N <= maxN elements with the fixed-size bitonic network of nv = maxN / L vectors
fn argsort_n_vec<const L: usize, const AVX512: bool, K: Kops>(v: &[K::K], ind: &mut [usize], n: usize, nv: usize) {
    if nv > 1 && n * 2 <= nv * L {
        argsort_n_vec::<L, AVX512, K>(v, ind, n, nv / 2);
        return;
    }
    let mut keys = vec![[K::type_max(); L]; nv];
    let mut idxs = vec![[usize::MAX; L]; nv];
    for i in 0..nv {
        let mask: [bool; L] = if i < nv / 2 {
            [true; L]
        } else {
            let num = n.saturating_sub(i * L).min(L);
            let mut m = [false; L];
            for l in 0..num {
                m[l] = true;
            }
            m
        };
        for l in 0..L {
            if mask[l] {
                idxs[i][l] = ind[i * L + l];
                keys[i][l] = v[idxs[i][l]];
            }
        }
        sort_reg_pair::<L, K>(&mut keys[i], &mut idxs[i]);
    }
    bitonic_fullmerge::<L, K>(&mut keys, &mut idxs);
    for i in 0..nv {
        let mask: [bool; L] = if i < nv / 2 {
            [true; L]
        } else {
            let num = n.saturating_sub(i * L).min(L);
            let mut m = [false; L];
            for l in 0..num {
                m[l] = true;
            }
            m
        };
        for l in 0..L {
            if mask[l] {
                ind[i * L + l] = idxs[i][l];
            }
        }
    }
}

fn get_pivot<const L: usize, K: Kops>(v: &[K::K], out: &[usize], left: usize, right: usize) -> K::K {
    if right - left >= L {
        let size = (right - left) / L;
        let mut r = [K::type_max(); L];
        for l in 0..L {
            r[l] = v[out[left + size * (l + 1)]];
        }
        sort_reg::<L, K>(&mut r);
        r[L / 2]
    } else {
        v[out[right]]
    }
}

fn partition_vec<const L: usize, const AVX512: bool, K: Kops>(
    v: &[K::K],
    out: &mut [usize],
    left: usize,
    right: usize, // exclusive end of the unpartitioned region
    arg_vec: &[usize; L],
    pivot: K::K,
) -> (usize, K::K, K::K) {
    let mut keys = [K::type_max(); L];
    let mut ge = [false; L];
    for l in 0..L {
        keys[l] = v[arg_vec[l]];
        ge[l] = !K::lt(keys[l], pivot);
    }
    let amount = ge.iter().filter(|x| **x).count();
    let minv = keys.iter().copied().fold(K::type_max(), |a, b| K::min2(a, b));
    let maxv = keys.iter().copied().fold(K::type_min(), |a, b| K::max2(a, b));
    if AVX512 {
        let mut p = left;
        for l in 0..L {
            if !ge[l] {
                out[p] = arg_vec[l];
                p += 1;
            }
        }
        let mut p = right - amount;
        for l in 0..L {
            if ge[l] {
                out[p] = arg_vec[l];
                p += 1;
            }
        }
    } else {
        // avx2_double_compressstore: write the same permuted register to both edges
        let mut perm = [0usize; L];
        let mut p = 0;
        for l in 0..L {
            if !ge[l] {
                perm[p] = arg_vec[l];
                p += 1;
            }
        }
        // the >= pivot lanes land at the end in reverse lane order
        for l in (0..L).rev() {
            if ge[l] {
                perm[p] = arg_vec[l];
                p += 1;
            }
        }
        for l in 0..L {
            out[left + l] = perm[l];
            out[right - L + l] = perm[l];
        }
    }
    (amount, minv, maxv)
}

fn argpartition_unrolled<const L: usize, const AVX512: bool, K: Kops>(
    v: &[K::K],
    out: &mut [usize],
    mut left: usize,
    mut right: usize, // exclusive
    pivot: K::K,
    smallest: &mut K::K,
    biggest: &mut K::K,
) -> usize {
    for _ in 0..(right - left) % (4 * L) {
        let val = v[out[left]];
        *smallest = K::min2(*smallest, val);
        *biggest = K::max2(*biggest, val);
        if !K::lt(val, pivot) {
            out.swap(left, right - 1);
            right -= 1;
        } else {
            left += 1;
        }
    }
    if left == right {
        return left;
    }
    let mut vec_left: [[usize; L]; 4] = [[0; L]; 4];
    let mut vec_right: [[usize; L]; 4] = [[0; L]; 4];
    for ii in 0..4 {
        for l in 0..L {
            vec_left[ii][l] = out[left + L * ii + l];
            vec_right[ii][l] = out[right - L * (4 - ii) + l];
        }
    }
    let mut r_store = right - L;
    let mut l_store = left;
    left += 4 * L;
    right -= 4 * L;
    let mut minv = *smallest;
    let mut maxv = *biggest;
    while right - left != 0 {
        let mut curr: [[usize; L]; 4] = [[0; L]; 4];
        if (r_store + L) - right < left - l_store {
            right -= 4 * L;
            for ii in 0..4 {
                for l in 0..L {
                    curr[ii][l] = out[right + L * ii + l];
                }
            }
        } else {
            for ii in 0..4 {
                for l in 0..L {
                    curr[ii][l] = out[left + L * ii + l];
                }
            }
            left += 4 * L;
        }
        for ii in 0..4 {
            let (amt, mn, mx) = partition_vec::<L, AVX512, K>(v, out, l_store, r_store + L, &curr[ii], pivot);
            l_store += L - amt;
            r_store -= amt;
            minv = K::min2(minv, mn);
            maxv = K::max2(maxv, mx);
        }
    }
    for ii in 0..4 {
        let (amt, mn, mx) = partition_vec::<L, AVX512, K>(v, out, l_store, r_store + L, &vec_left[ii], pivot);
        l_store += L - amt;
        r_store -= amt;
        minv = K::min2(minv, mn);
        maxv = K::max2(maxv, mx);
    }
    for ii in 0..4 {
        let (amt, mn, mx) = partition_vec::<L, AVX512, K>(v, out, l_store, r_store + L, &vec_right[ii], pivot);
        l_store += L - amt;
        r_store -= amt;
        minv = K::min2(minv, mn);
        maxv = K::max2(maxv, mx);
    }
    *smallest = minv;
    *biggest = maxv;
    l_store
}

// Returns false when numpy would hand the segment to std::sort (only when the recursion
// exhausts its budget); the caller refuses rather than guess that order.
fn argsort_<const L: usize, const AVX512: bool, K: Kops>(
    v: &[K::K],
    out: &mut [usize],
    left: usize,
    right: usize,
    max_iters: usize,
) -> bool {
    if max_iters == 0 {
        return false;
    }
    let len = right + 1 - left;
    if len <= 256 {
        argsort_n_vec::<L, AVX512, K>(v, &mut out[left..right + 1], len, 256 / L);
        return true;
    }
    let pivot = get_pivot::<L, K>(v, out, left, right);
    let mut smallest = K::type_max();
    let mut biggest = K::type_min();
    let pi = argpartition_unrolled::<L, AVX512, K>(v, out, left, right + 1, pivot, &mut smallest, &mut biggest);
    if pivot != smallest && !argsort_::<L, AVX512, K>(v, out, left, pi - 1, max_iters - 1) {
        return false;
    }
    if pivot != biggest && !argsort_::<L, AVX512, K>(v, out, pi, right, max_iters - 1) {
        return false;
    }
    true
}

fn xss_argsort<const L: usize, const AVX512: bool, K: Kops>(v: &[K::K], out: &mut [usize]) -> bool {
    let n = v.len();
    if n <= 1 {
        return true;
    }
    // numpy leaves an already non-decreasing array untouched (identity indices)
    if !v.windows(2).any(|w| K::lt(w[1], w[0])) {
        return true;
    }
    let max_iters = (2.0 * (n as f64).log2()) as usize;
    argsort_::<L, AVX512, K>(v, out, 0, n - 1, max_iters)
}

// ---------- numpy's dispatch: X86_V4 first, then X86_V3, else introsort ----------

#[derive(Clone, Copy, PartialEq)]
enum NpPath {
    Intro,
    Avx2,
    Avx512,
}

#[cfg(target_arch = "x86_64")]
fn x86_v2() -> bool {
    use std::arch::x86_64::__cpuid;
    let l1 = unsafe { __cpuid(1) };
    let cx16 = l1.ecx & (1 << 13) != 0;
    let ext = unsafe { __cpuid(0x8000_0001) };
    let lahf = ext.ecx & 1 != 0;
    std::arch::is_x86_feature_detected!("sse")
        && std::arch::is_x86_feature_detected!("sse2")
        && std::arch::is_x86_feature_detected!("sse3")
        && std::arch::is_x86_feature_detected!("ssse3")
        && std::arch::is_x86_feature_detected!("sse4.1")
        && std::arch::is_x86_feature_detected!("sse4.2")
        && std::arch::is_x86_feature_detected!("popcnt")
        && lahf
        && cx16
}
#[cfg(target_arch = "x86_64")]
fn x86_v3() -> bool {
    x86_v2()
        && std::arch::is_x86_feature_detected!("avx")
        && std::arch::is_x86_feature_detected!("avx2")
        && std::arch::is_x86_feature_detected!("fma")
        && std::arch::is_x86_feature_detected!("bmi1")
        && std::arch::is_x86_feature_detected!("bmi2")
        && std::arch::is_x86_feature_detected!("lzcnt")
        && std::arch::is_x86_feature_detected!("f16c")
        && std::arch::is_x86_feature_detected!("movbe")
}
#[cfg(target_arch = "x86_64")]
fn x86_v4() -> bool {
    x86_v3()
        && std::arch::is_x86_feature_detected!("avx512f")
        && std::arch::is_x86_feature_detected!("avx512cd")
        && std::arch::is_x86_feature_detected!("avx512bw")
        && std::arch::is_x86_feature_detected!("avx512dq")
        && std::arch::is_x86_feature_detected!("avx512vl")
}
#[cfg(not(target_arch = "x86_64"))]
fn x86_v2() -> bool {
    false
}
#[cfg(not(target_arch = "x86_64"))]
fn x86_v3() -> bool {
    false
}
#[cfg(not(target_arch = "x86_64"))]
fn x86_v4() -> bool {
    false
}

fn np_path() -> NpPath {
    if x86_v4() {
        NpPath::Avx512
    } else if x86_v3() {
        NpPath::Avx2
    } else {
        NpPath::Intro
    }
}

// Fill out with numpy's default-kind argsort of v. false = this path is not reproduced
// (the caller keeps its refusal): only when numpy would use std::sort inside x86-simd-sort.
pub fn np_default_argsort_i64(v: &[i64], out: &mut [usize]) -> bool {
    match np_path() {
        NpPath::Avx512 => xss_argsort::<8, true, i64>(v, out),
        NpPath::Avx2 => xss_argsort::<4, false, i64>(v, out),
        NpPath::Intro => {
            intro_argsort::<i64>(v, out);
            true
        }
    }
}
pub fn np_default_argsort_i32(v: &[i32], out: &mut [usize]) -> bool {
    match np_path() {
        NpPath::Avx512 => xss_argsort::<8, true, i32>(v, out),
        NpPath::Avx2 => xss_argsort::<4, false, i32>(v, out),
        NpPath::Intro => {
            intro_argsort::<i32>(v, out);
            true
        }
    }
}
pub fn np_default_argsort_f64(v: &[f64], out: &mut [usize]) -> bool {
    match np_path() {
        NpPath::Avx512 | NpPath::Avx2 => {
            if f64::any_nan(v) {
                false
            } else if x86_v4() {
                xss_argsort::<8, true, f64>(v, out)
            } else {
                xss_argsort::<4, false, f64>(v, out)
            }
        }
        NpPath::Intro => {
            intro_argsort::<f64>(v, out);
            true
        }
    }
}
pub fn np_default_argsort_f32(v: &[f32], out: &mut [usize]) -> bool {
    match np_path() {
        NpPath::Avx512 | NpPath::Avx2 => {
            if f32::any_nan(v) {
                false
            } else if x86_v4() {
                xss_argsort::<8, true, f32>(v, out)
            } else {
                xss_argsort::<4, false, f32>(v, out)
            }
        }
        NpPath::Intro => {
            intro_argsort::<f32>(v, out);
            true
        }
    }
}


// ---------- numpy.linalg, histogram, cov/corrcoef, trapezoid, ufunc reduce/accumulate ----------
// Built from numpy's own Python-level definitions (numpy/linalg/_linalg.py, numpy/lib/_histograms_impl.py,
// ...) over the native pieces, so the steps and their order are numpy's. LAPACK (det, solve, inv)
// follows OpenBLAS's own code (see lk_getrf below).

fn np_call(n: &str, p: Vec<V>, k: Vec<(Sym, V)>) -> R {
    np_module_call("numpy", n, Args { p, k })
}
fn np_meth(v: &V, n: &str, p: Vec<V>, k: Vec<(Sym, V)>) -> R {
    call_method(v, n, Args { p, k })
}
fn np_float_arr(v: &V) -> Result<NdArr, E> {
    let a = np_as(v)?;
    Ok(if a.dt() == NT::F { a } else { NdArr::new(a.vals().cast(NT::F), a.shape.clone()) })
}
fn linalg_err(m: &str) -> E {
    exc("LinAlgError", m.into())
}
fn square_check(a: &NdArr) -> Result<usize, E> {
    if a.ndim() < 2 {
        return Err(linalg_err(&format!("{}-dimensional array given. Array must be at least two-dimensional", a.ndim())));
    }
    if a.ndim() > 2 {
        return Err(np_unsup("np.linalg on stacks of matrices"));
    }
    if a.shape[0] != a.shape[1] {
        return Err(linalg_err("Last 2 dimensions of the array must be square"));
    }
    Ok(a.shape[0])
}
fn fchain(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        s = a[i].mul_add(b[i], s);
    }
    s
}
// ---------- LAPACK as OpenBLAS runs it (numpy's det, slogdet, solve, inv) ----------
// numpy calls OpenBLAS's dgetrf and dgesv. Their result depends on the order the kernels add in, so this
// follows OpenBLAS (0.3.30) step by step on one thread: getrf_single.c (recursive blocking), getf2_k.c,
// dgemv_n_4.c, the dgemm kernels' k-loops (dgemm_kernel_16x2_skylakex.c, dgemm_kernel_4x8_haswell.S),
// trsm_kernel_LT/LN.c, trsv_L/U.c and daxpy.c, for the SkylakeX (AVX-512) and Haswell (AVX2) kernels.
// numpy itself runs this on one thread for solve and inv below 100 x 100 and for det below about
// 140 x 140; larger ones it splits across threads, which adds in another order.
// Matrices here are column-major (LAPACK's), element (i, j) at i + j * ld.
struct Lk {
    skx: bool,
    um: usize,
    un: usize,
    q: usize,
}
fn lk() -> Lk {
    if np_blas() == Blas::Skx {
        Lk { skx: true, um: 16, un: 2, q: 384 }
    } else {
        Lk { skx: false, um: 4, un: 8, q: 256 }
    }
}
fn lk_blas(k: &Lk) -> Blas {
    if k.skx { Blas::Skx } else { Blas::Hsw }
}
// y -= A x for the m x n block of `a` at (r0, c0) (dgemv_n_4.c, alpha = -1)
fn lk_gemv_n(k: &Lk, a: &[f64], lda: usize, r0: usize, c0: usize, m: usize, n: usize, x: &[f64], y: &mut [f64]) {
    if m == 0 || n == 0 {
        return;
    }
    let at = |r: usize, c: usize| a[r0 + r + (c0 + c) * lda];
    let mb = m - (m & 3);
    for r in 0..mb {
        let mut c = 0;
        while c + 4 <= n {
            let t = if k.skx {
                let t = fma(at(r, c), x[c], at(r, c + 1) * x[c + 1]);
                let t = fma(at(r, c + 2), x[c + 2], t);
                fma(at(r, c + 3), x[c + 3], t)
            } else {
                fma(at(r, c + 2), x[c + 2], at(r, c) * x[c]) + fma(at(r, c + 3), x[c + 3], at(r, c + 1) * x[c + 1])
            };
            y[r] -= t;
            c += 4;
        }
        if n - c >= 2 {
            let t = if k.skx { fma(at(r, c), x[c], at(r, c + 1) * x[c + 1]) } else { at(r, c) * x[c] + at(r, c + 1) * x[c + 1] };
            y[r] -= t;
            c += 2;
        }
        if n - c >= 1 {
            y[r] += at(r, c) * -x[c];
        }
    }
    for r in mb..m {
        let mut t = 0.0;
        for c in 0..n {
            t = if k.skx { fma(at(r, c), x[c], t) } else { t + at(r, c) * x[c] };
        }
        y[r] -= t;
    }
}
fn lk_col_chunks(k: &Lk, n: usize) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    if k.skx {
        for sz in [12, 10, 8, 6, 4, 2, 1] {
            while n - i >= sz {
                out.push((i, sz));
                i += sz;
            }
        }
    } else {
        while n - i >= 24 {
            out.push((i, 12));
            out.push((i + 12, 12));
            i += 24;
        }
        while n - i >= 8 {
            out.push((i, 8));
            i += 8;
        }
        for sz in [4, 2, 1] {
            if n - i >= sz {
                out.push((i, sz));
                i += sz;
            }
        }
    }
    out
}
fn lk_row_chunks(k: &Lk, m: usize) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    let big = if k.skx { 16 } else { 4 };
    while m - i >= big {
        out.push((i, big));
        i += big;
    }
    let tails: &[usize] = if k.skx { &[8, 4, 2, 1] } else { &[2, 1] };
    for &sz in tails {
        if m - i >= sz {
            out.push((i, sz));
            i += sz;
        }
    }
    out
}
// C -= A B through one dgemm kernel call: C is rows r0.. x cols cols of `c`, B(t, col) = c[kr0 + t + col * ldc],
// A(r, t) = ablk[r * kn + t] (copied out beforehand)
fn lk_gemm(k: &Lk, c: &mut [f64], ldc: usize, r0: usize, rn: usize, c0: usize, cn: usize, kr0: usize, kn: usize, ablk: &[f64]) {
    if rn == 0 || cn == 0 || kn == 0 {
        return;
    }
    for (cc, cs) in lk_col_chunks(k, cn) {
        for (rr, rs) in lk_row_chunks(k, rn) {
            let w = if k.skx && cs == 12 {
                match rs {
                    4 => 2,
                    2 | 1 => 4,
                    _ => 1,
                }
            } else {
                1
            };
            let hsw41 = !k.skx && rs == 4 && cs == 1;
            for r in rr..rr + rs {
                let ar = &ablk[r * kn..(r + 1) * kn];
                for col in c0 + cc..c0 + cc + cs {
                    let b = &c[kr0 + col * ldc..kr0 + col * ldc + kn];
                    let acc = if hsw41 {
                        let mut q = [0.0f64; 4];
                        let k8 = kn / 8 * 8;
                        for t in 0..k8 {
                            q[t % 4] = fma(ar[t], b[t], q[t % 4]);
                        }
                        for t in k8..kn {
                            q[0] = fma(ar[t], b[t], q[0]);
                        }
                        (q[0] + q[1]) + (q[2] + q[3])
                    } else if w > 1 && kn >= w {
                        let mut q = [0.0f64; 4];
                        let mut t = 0;
                        while kn - t >= w {
                            for l in 0..w {
                                q[l] = fma(ar[t + l], b[t + l], q[l]);
                            }
                            t += w;
                        }
                        let mut acc = if w == 2 { q[0] + q[1] } else { (q[0] + q[1]) + (q[2] + q[3]) };
                        while t < kn {
                            acc = fma(ar[t], b[t], acc);
                            t += 1;
                        }
                        acc
                    } else {
                        let mut acc = 0.0;
                        for t in 0..kn {
                            acc = fma(ar[t], b[t], acc);
                        }
                        acc
                    };
                    c[r0 + r + col * ldc] -= acc;
                }
            }
        }
    }
}
// copy the block of `a` at (r0, k0), rn x kn, row by row
fn lk_block(a: &[f64], lda: usize, r0: usize, rn: usize, k0: usize, kn: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(rn * kn);
    for r in 0..rn {
        for t in 0..kn {
            out.push(a[r0 + r + (k0 + t) * lda]);
        }
    }
    out
}
// TRSM_KERNEL_LT: solve L X = C for the unit lower triangle L of `l` at (lr0, lr0), jb rows; C is rows
// cr0..cr0+jb x cols (c0..c0+cn) of `c`.
fn lk_strips(k: &Lk, n: usize) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut c = 0;
    while n - c >= k.un {
        out.push((c, k.un));
        c += k.un;
    }
    let mut j = k.un >> 1;
    while j > 0 {
        if (n - c) & j != 0 {
            out.push((c, j));
            c += j;
        }
        j >>= 1;
    }
    out
}
fn lk_sub(k: &Lk, c: f64, b: f64, a: f64) -> f64 {
    if k.skx { fma(-b, a, c) } else { c - b * a }
}
fn lk_trsm_lt(k: &Lk, l: &[f64], ldl: usize, lr0: usize, jb: usize, c: &mut [f64], ldc: usize, cr0: usize, c0: usize, cn: usize) {
    let mut blocks = vec![];
    let mut i = 0;
    while jb - i >= k.um {
        blocks.push((i, k.um));
        i += k.um;
    }
    let mut s = k.um >> 1;
    while s > 0 {
        if jb & s != 0 {
            blocks.push((i, s));
            i += s;
        }
        s >>= 1;
    }
    let lat = |r: usize, t: usize| l[lr0 + r + (lr0 + t) * ldl];
    let abs: Vec<Vec<f64>> = blocks.iter().map(|&(b0, bs)| lk_block(l, ldl, lr0 + b0, bs, lr0, b0)).collect();
    for (sc, sw) in lk_strips(k, cn) {
        for (bi, &(b0, bs)) in blocks.iter().enumerate() {
            if b0 > 0 {
                lk_gemm(k, c, ldc, cr0 + b0, bs, c0 + sc, sw, cr0, b0, &abs[bi]);
            }
            for ii in 0..bs {
                let ri = b0 + ii;
                for col in c0 + sc..c0 + sc + sw {
                    let bb = c[cr0 + ri + col * ldc];
                    for kk in ii + 1..bs {
                        let rk = b0 + kk;
                        let v = &mut c[cr0 + rk + col * ldc];
                        *v = lk_sub(k, *v, bb, lat(rk, ri));
                    }
                }
            }
        }
    }
}
// TRSM_KERNEL_LN: solve U X = C for the upper triangle of `u` (n x n), C = all n rows x cols of `c`
fn lk_trsm_ln(k: &Lk, u: &[f64], n: usize, c: &mut [f64], ldc: usize, c0: usize, cn: usize) {
    let mut blocks = vec![];
    let mut i = 1;
    while i < k.um {
        if n & i != 0 {
            blocks.push(((n & !(i - 1)) - i, i));
        }
        i *= 2;
    }
    let mut r = (n & !(k.um - 1)) as isize - k.um as isize;
    while r >= 0 {
        blocks.push((r as usize, k.um));
        r -= k.um as isize;
    }
    let abs: Vec<Vec<f64>> = blocks.iter().map(|&(b0, bs)| lk_block(u, n, b0, bs, b0 + bs, n - b0 - bs)).collect();
    for (sc, sw) in lk_strips(k, cn) {
        for (bi, &(b0, bs)) in blocks.iter().enumerate() {
            if b0 + bs < n {
                lk_gemm(k, c, ldc, b0, bs, c0 + sc, sw, b0 + bs, n - b0 - bs, &abs[bi]);
            }
            for ii in (0..bs).rev() {
                let ri = b0 + ii;
                let inv = 1.0 / u[ri + ri * n];
                for col in c0 + sc..c0 + sc + sw {
                    let bb = c[ri + col * ldc] * inv;
                    c[ri + col * ldc] = bb;
                    for kk in 0..ii {
                        let rk = b0 + kk;
                        let v = &mut c[rk + col * ldc];
                        *v = lk_sub(k, *v, bb, u[rk + ri * n]);
                    }
                }
            }
        }
    }
}
fn lk_laswp(a: &mut [f64], lda: usize, c0: usize, cn: usize, k1: usize, k2: usize, ipiv: &[usize]) {
    for i in k1..k2 {
        let ip = ipiv[i];
        if ip != i {
            for col in c0..c0 + cn {
                a.swap(i + col * lda, ip + col * lda);
            }
        }
    }
}
// getf2_k.c on the panel of rows off..m, cols off..nend; returns LAPACK's info (first zero pivot, 1-based)
fn lk_getf2(k: &Lk, a: &mut [f64], lda: usize, ipiv: &mut [usize], off: usize, mt: usize, nend: usize) -> usize {
    let (m, n) = (mt - off, nend - off);
    let mut info = 0;
    let bl = lk_blas(k);
    let mut b = vec![0.0; m];
    let mut row = vec![0.0; n];
    for j in 0..n {
        let cj = off + j;
        for i in 0..m {
            b[i] = a[off + i + cj * lda];
        }
        let len = j.min(m);
        for i in 0..len {
            let ip = ipiv[i + off] - off;
            if ip != i {
                b.swap(i, ip);
            }
        }
        for i in 1..len {
            for t in 0..i {
                row[t] = a[off + i + (off + t) * lda];
            }
            b[i] -= ddot1(&row[..i], &b[..i], false, bl);
        }
        if j < m {
            let (head, tail) = b.split_at_mut(j);
            lk_gemv_n(k, a, lda, off + j, off, m - j, j, head, tail);
        }
        for i in 0..m {
            a[off + i + cj * lda] = b[i];
        }
        if j < m {
            let mut jp = j;
            for i in j + 1..m {
                if b[i].abs() > b[jp].abs() {
                    jp = i;
                }
            }
            ipiv[off + j] = jp + off;
            let t1 = b[jp];
            if t1 != 0.0 {
                if t1.abs() >= f64::MIN_POSITIVE {
                    let r = 1.0 / t1;
                    if jp != j {
                        for col in off..off + j + 1 {
                            a.swap(off + j + col * lda, off + jp + col * lda);
                        }
                    }
                    for i in j + 1..m {
                        a[off + i + cj * lda] *= r;
                    }
                }
            } else if info == 0 {
                info = j + 1;
            }
        }
    }
    info
}
fn lk_getrf_rec(k: &Lk, a: &mut [f64], lda: usize, ipiv: &mut [usize], off: usize, mt: usize, nend: usize) -> usize {
    let (m, n) = (mt.saturating_sub(off), nend - off);
    if m == 0 || n == 0 {
        return 0;
    }
    let mn = m.min(n);
    let blocking = ((mn / 2 + k.un - 1) / k.un * k.un).min(k.q);
    if blocking <= 2 * k.un {
        return lk_getf2(k, a, lda, ipiv, off, mt, nend);
    }
    let mut info = 0;
    let mut j = 0;
    while j < mn {
        let jb = (mn - j).min(blocking);
        let ii = lk_getrf_rec(k, a, lda, ipiv, off + j, mt, off + j + jb);
        if ii != 0 && info == 0 {
            info = ii + j;
        }
        if j + jb < n {
            let (c0, cn) = (off + j + jb, n - j - jb);
            lk_laswp(a, lda, c0, cn, off + j, off + j + jb, ipiv);
            let l = lk_block(a, lda, off + j, jb, off + j, jb);
            let lt: Vec<f64> = (0..jb * jb).map(|i| l[(i % jb) * jb + i / jb]).collect();
            lk_trsm_lt(k, &lt, jb, 0, jb, a, lda, off + j, c0, cn);
            let rn = mt - (off + j + jb);
            let ab = lk_block(a, lda, off + j + jb, rn, off + j, jb);
            lk_gemm(k, a, lda, off + j + jb, rn, c0, cn, off + j, jb, &ab);
        }
        j += blocking;
    }
    let mut j = 0;
    while j < mn {
        let jb = (mn - j).min(blocking);
        lk_laswp(a, lda, off + j, jb, off + j + jb, off + mn, ipiv);
        j += jb;
    }
    info
}
// dgetrf on a column-major n x n matrix: (pivot rows, info)
fn lk_getrf(a: &mut [f64], n: usize) -> (Vec<usize>, usize) {
    let k = lk();
    let mut ipiv = vec![0; n];
    let info = lk_getrf_rec(&k, a, n, &mut ipiv, 0, n, n);
    (ipiv, info)
}
// daxpy: y += da * x
fn lk_axpy(k: &Lk, da: f64, x: &[f64], y: &mut [f64]) {
    let n1 = x.len() & !15;
    for i in 0..x.len() {
        y[i] = if i < n1 || k.skx { fma(da, x[i], y[i]) } else { y[i] + da * x[i] };
    }
}
// dgetrs after dgetrf: b is column-major n x m (overwritten with X)
fn lk_getrs(lu: &[f64], ipiv: &[usize], n: usize, b: &mut [f64], m: usize) {
    let k = lk();
    lk_laswp(b, n, 0, m, 0, n, ipiv);
    if m == 1 {
        const DTB: usize = 64;
        let mut col = vec![0.0; n];
        let mut is = 0;
        while is < n {
            let mi = (n - is).min(DTB);
            for i in 0..mi {
                let r = is + i;
                if i + 1 < mi {
                    for t in r + 1..is + mi {
                        col[t] = lu[t + r * n];
                    }
                    let br = -b[r];
                    lk_axpy(&k, br, &col[r + 1..is + mi], &mut b[r + 1..is + mi]);
                }
            }
            if n - is > mi {
                let (head, tail) = b.split_at_mut(is + mi);
                lk_gemv_n(&k, lu, n, is + mi, is, n - is - mi, mi, &head[is..], tail);
            }
            is += DTB;
        }
        let mut is = n;
        while is > 0 {
            let mi = is.min(DTB);
            let lo = is - mi;
            for i in 0..mi {
                let r = is - i - 1;
                b[r] /= lu[r + r * n];
                if i + 1 < mi {
                    for t in lo..r {
                        col[t] = lu[t + r * n];
                    }
                    let br = -b[r];
                    lk_axpy(&k, br, &col[lo..r], &mut b[lo..r]);
                }
            }
            if lo > 0 {
                let (head, tail) = b.split_at_mut(lo);
                lk_gemv_n(&k, lu, n, 0, lo, lo, mi, &tail[..mi], head);
            }
            is -= mi;
        }
        return;
    }
    lk_trsm_lt(&k, lu, n, 0, n, b, n, 0, 0, m);
    lk_trsm_ln(&k, lu, n, b, n, 0, m);
}
// row-major n x m <-> column-major
fn lk_colmajor(a: &[f64], n: usize, m: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * m];
    for i in 0..n {
        for j in 0..m {
            out[i + j * n] = a[i * m + j];
        }
    }
    out
}
fn lk_rowmajor(a: &[f64], n: usize, m: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * m];
    for i in 0..n {
        for j in 0..m {
            out[i * m + j] = a[i + j * n];
        }
    }
    out
}
fn np_det_parts(a: &NdArr) -> Result<(f64, f64), E> {
    let n = square_check(a)?;
    if n == 0 {
        return Ok((1.0, 0.0));
    }
    let mut m = lk_colmajor(&a.fv(), n, n);
    let (piv, info) = lk_getrf(&mut m, n);
    if info != 0 {
        return Ok((0.0, f64::NEG_INFINITY));
    }
    // numpy's slogdet_from_factored_diagonal
    let mut sign = if piv.iter().enumerate().filter(|(j, p)| *j != **p).count() % 2 == 1 { -1.0 } else { 1.0 };
    let mut logdet = 0.0;
    for i in 0..n {
        let mut d = m[i + i * n];
        if d < 0.0 {
            sign = -sign;
            d = -d;
        }
        logdet += d.ln();
    }
    Ok((sign, logdet))
}

pub fn np_linalg_call(n: &str, a: &mut Args) -> R {
    let a = &*a;
    let first = || -> Result<V, E> { a.p.first().cloned().ok_or_else(|| exc("TypeError", format!("{n}() missing required argument"))) };
    match n {
        "det" => {
            let x = np_float_arr(&first()?)?;
            let (s, l) = np_det_parts(&x)?;
            Ok(np_sc(Sc::F(s * l.exp())))
        }
        "slogdet" => {
            let x = np_float_arr(&first()?)?;
            let (sg, l) = np_det_parts(&x)?;
            thread_local! { static SLOGDET: RefCell<Option<V>> = RefCell::new(None); }
            let c = match SLOGDET.with(|c| c.borrow().clone()) {
                Some(c) => c,
                None => {
                    let c = namedtuple(Args::p(vec![s("SlogdetResult"), s("sign logabsdet")]))?;
                    SLOGDET.with(|x| *x.borrow_mut() = Some(c.clone()));
                    c
                }
            };
            call(&c, Args::p(vec![np_sc(Sc::F(sg)), np_sc(Sc::F(l))]))
        }
        "solve" | "inv" => {
            let x = np_float_arr(&first()?)?;
            let k = square_check(&x)?;
            let mut lu = x.fv();
            let (bv, bshape, m) = if n == "inv" {
                let mut e = vec![0.0; k * k];
                for i in 0..k {
                    e[i * k + i] = 1.0;
                }
                (e, vec![k, k], k)
            } else {
                let b = np_float_arr(&a.p.get(1).cloned().ok_or_else(|| exc("TypeError", "solve() missing required argument 'b' (pos 1)".into()))?)?;
                match b.ndim() {
                    1 if b.shape[0] == k => (b.fv(), vec![k], 1),
                    2 if b.shape[0] == k => (b.fv(), b.shape.clone(), b.shape[1]),
                    1 | 2 => {
                        return val_err(format!("solve1: Input operand 1 has a mismatch in its core dimension 0, with gufunc signature (m,m),(m)->(m) (size {} is different from {k})", b.shape[0]));
                    }
                    _ => return Err(np_unsup("np.linalg.solve on stacks")),
                }
            };
            let mut lu = lk_colmajor(&lu, k, k);
            let (piv, info) = lk_getrf(&mut lu, k);
            if info != 0 {
                return Err(linalg_err("Singular matrix"));
            }
            let mut b = lk_colmajor(&bv, k, m);
            lk_getrs(&lu, &piv, k, &mut b, m);
            Ok(np_new(Buf::F(lk_rowmajor(&b, k, m)), bshape))
        }
        "matrix_power" => {
            let x = np_as(&first()?)?;
            let k = np_int_arg(a.p.get(1).ok_or_else(|| exc("TypeError", "matrix_power() missing required argument 'n' (pos 1)".into()))?)?;
            square_check(&x)?;
            let mut base = np_arr(x.clone());
            let mut p = k;
            if p == 0 {
                let e = np_call("eye", vec![V::Int(x.shape[0] as i64)], vec![])?;
                return np_meth(&e, "astype", vec![np_dtype_v(x.dt())], vec![]);
            }
            if p < 0 {
                base = np_linalg_call("inv", &mut Args::p(vec![base]))?;
                p = -p;
            }
            if p == 1 {
                return Ok(np_arr(as_np(&base).unwrap().copy()));
            }
            if p == 2 {
                return np_matmul(&base, &base);
            }
            if p == 3 {
                return np_matmul(&np_matmul(&base, &base)?, &base);
            }
            let (mut z, mut result): (Option<V>, Option<V>) = (None, None);
            while p > 0 {
                z = Some(match z {
                    None => base.clone(),
                    Some(z) => np_matmul(&z, &z)?,
                });
                let bit = p % 2;
                p /= 2;
                if bit == 1 {
                    let zz = z.clone().unwrap();
                    result = Some(match result {
                        None => zz,
                        Some(r) => np_matmul(&r, &zz)?,
                    });
                }
            }
            Ok(result.unwrap())
        }
        "norm" => np_norm(a),
        "lstsq" => np_lstsq(a),
        _ => Err(np_unsup(&format!("np.linalg.{n}"))),
    }
}
fn np_dtype_v(d: NT) -> V {
    V::Ext(Rc::new(NpDtype(d)))
}
fn np_norm(a: &Args) -> R {
    let x0 = npa(a, 0, "x").ok_or_else(|| exc("TypeError", "norm() missing required argument 'x' (pos 0)".into()))?;
    let mut x = np_float_arr(&x0)?;
    let ord = npa(a, 1, "ord").filter(|v| !matches!(v, V::None));
    let axis = npa(a, 2, "axis").filter(|v| !matches!(v, V::None));
    let keepdims = npa(a, 3, "keepdims").map_or(Ok(false), |v| truth(&v))?;
    let ord_s = ord.as_ref().and_then(|v| if let V::Str(s) = v { Some(s.s.clone()) } else { None });
    let ord_f = match (&ord, &ord_s) {
        (Some(v), None) => Some(np_f_arg(v)?),
        _ => None,
    };
    let nd = x.ndim();
    if axis.is_none() {
        let fro = ord.is_none() || (matches!(ord_s.as_deref(), Some("f" | "fro")) && nd == 2) || (ord_f == Some(2.0) && nd == 1);
        if fro {
            if !x.contig() {
                x = x.copy();
            }
            let v = x.fv();
            let r = np_ddot(&v, &v, true).sqrt();
            if keepdims {
                return Ok(np_new(Buf::F(vec![r]), vec![1; nd]));
            }
            return Ok(np_sc(Sc::F(r)));
        }
    }
    let axes: Vec<i64> = match &axis {
        None => (0..nd as i64).collect(),
        Some(V::Tuple(t)) => t.iter().map(np_int_arg).collect::<Result<_, E>>()?,
        Some(v) => vec![np_int_arg(v)?],
    };
    let xv = np_arr(x.clone());
    let absx = || np_call("abs", vec![xv.clone()], vec![]);
    let kd = |k: Vec<(Sym, V)>| -> Vec<(Sym, V)> {
        let mut k = k;
        k.push(("keepdims", V::Bool(keepdims)));
        k
    };
    if axes.len() == 1 {
        let ax = V::Int(axes[0]);
        if let Some(s) = &ord_s {
            return val_err(format!("Invalid norm order '{s}' for vectors"));
        }
        let o = ord_f;
        return match o {
            Some(f) if f == f64::INFINITY => np_meth(&absx()?, "max", vec![], kd(vec![("axis", ax)])),
            Some(f) if f == f64::NEG_INFINITY => np_meth(&absx()?, "min", vec![], kd(vec![("axis", ax)])),
            Some(f) if f == 0.0 => {
                let nz = np_binop(&xv, &V::Int(0), "!=").unwrap()?;
                let nz = np_meth(&nz, "astype", vec![np_dtype_v(NT::F)], vec![])?;
                np_meth(&nz, "sum", vec![], kd(vec![("axis", ax)]))
            }
            Some(f) if f == 1.0 => np_meth(&absx()?, "sum", vec![], kd(vec![("axis", ax)])),
            None => {
                let s = np_binop(&xv, &xv, "*").unwrap()?;
                np_call("sqrt", vec![np_meth(&s, "sum", vec![], kd(vec![("axis", ax)]))?], vec![])
            }
            Some(f) if f == 2.0 => {
                let s = np_binop(&xv, &xv, "*").unwrap()?;
                np_call("sqrt", vec![np_meth(&s, "sum", vec![], kd(vec![("axis", ax)]))?], vec![])
            }
            Some(f) => {
                let p = np_binop(&absx()?, &V::Float(f), "**").unwrap()?;
                let r = np_meth(&p, "sum", vec![], kd(vec![("axis", ax)]))?;
                np_binop(&r, &V::Float(1.0 / f), "**").unwrap()
            }
        };
    }
    if axes.len() == 2 {
        let r0 = norm_axis(axes[0], nd)?;
        let r1 = norm_axis(axes[1], nd)?;
        if r0 == r1 {
            return val_err("Duplicate axes given.".into());
        }
        let (mut row, mut colx) = (r0 as i64, r1 as i64);
        let r = match (ord_f, ord_s.as_deref()) {
            (None, None) | (None, Some("f" | "fro")) => {
                let s = np_binop(&xv, &xv, "*").unwrap()?;
                if nd == 2 {
                    let t = np_meth(&s, "sum", vec![], vec![])?;
                    np_call("sqrt", vec![t], vec![])?
                } else {
                    return Err(np_unsup("np.linalg.norm over two axes of a 3-D array"));
                }
            }
            (Some(f), None) if f == 1.0 || f == -1.0 => {
                if colx > row {
                    colx -= 1;
                }
                let s = np_meth(&absx()?, "sum", vec![], vec![("axis", V::Int(row))])?;
                np_meth(&s, if f == 1.0 { "max" } else { "min" }, vec![], vec![("axis", V::Int(colx))])?
            }
            (Some(f), None) if f.is_infinite() => {
                if row > colx {
                    row -= 1;
                }
                let s = np_meth(&absx()?, "sum", vec![], vec![("axis", V::Int(colx))])?;
                np_meth(&s, if f > 0.0 { "max" } else { "min" }, vec![], vec![("axis", V::Int(row))])?
            }
            (Some(f), None) if f == 2.0 || f == -2.0 => return Err(np_unsup("np.linalg.norm(ord=2) of a matrix (singular values)")),
            (None, Some("nuc")) => return Err(np_unsup("np.linalg.norm(ord='nuc')")),
            _ => return val_err("Invalid norm order for matrices.".into()),
        };
        if keepdims {
            let mut sh = x.shape.clone();
            sh[r0] = 1;
            sh[r1] = 1;
            return np_meth(&r, "reshape", vec![tup(sh.iter().map(|v| V::Int(*v as i64)).collect())], vec![]);
        }
        return Ok(r);
    }
    val_err("Improper number of dimensions to norm.".into())
}

// np.histogram(a, bins=10, range=None, density=None, weights=None)
pub fn np_histogram(a: &Args) -> R {
    let x0 = npa(a, 0, "a").ok_or_else(|| exc("TypeError", "histogram() missing required argument 'a' (pos 0)".into()))?;
    let x = np_as(&x0)?;
    let x = NdArr::new(x.vals(), vec![x.size()]);
    if npa(a, 4, "weights").is_some_and(|v| !matches!(v, V::None)) {
        return Err(np_unsup("np.histogram(weights=...)"));
    }
    let density = npa(a, 3, "density").map_or(Ok(false), |v| truth(&v))?;
    let bins = npa(a, 1, "bins").unwrap_or(V::Int(10));
    let range = npa(a, 2, "range").filter(|v| !matches!(v, V::None));
    let xf = x.fv();
    let mut edge_arr: Option<NdArr> = None;
    let (counts, edges): (Vec<i64>, Vec<f64>) = if matches!(bins, V::Int(_)) || as_npsc(&bins).is_some_and(|s| s.dt() == NT::I) {
        let nb = np_int_arg(&bins)?;
        if nb < 1 {
            return val_err("`bins` must be positive, when an integer".into());
        }
        let nb = nb as usize;
        let (mut lo, mut hi) = match &range {
            Some(r) => {
                let v = to_vec(r)?;
                if v.len() != 2 {
                    return val_err("too many values to unpack (expected 2)".into());
                }
                let (l, h) = (np_f_arg(&v[0])?, np_f_arg(&v[1])?);
                if l > h {
                    return val_err("max must be larger than min in range parameter.".into());
                }
                if !(l.is_finite() && h.is_finite()) {
                    return val_err(format!("supplied range of [{}, {}] is not finite", np_sc_str(Sc::F(l)), np_sc_str(Sc::F(h))));
                }
                (l, h)
            }
            None if xf.is_empty() => (0.0, 1.0),
            None => {
                let (mut l, mut h) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in &xf {
                    if v.is_nan() {
                        return val_err("autodetected range of [nan, nan] is not finite".into());
                    }
                    l = l.min(*v);
                    h = h.max(*v);
                }
                if !(l.is_finite() && h.is_finite()) {
                    return val_err(format!("autodetected range of [{}, {}] is not finite", np_sc_str(Sc::F(l)), np_sc_str(Sc::F(h))));
                }
                (l, h)
            }
        };
        if lo == hi {
            lo -= 0.5;
            hi += 0.5;
        }
        let ev = np_call("linspace", vec![V::Float(lo), V::Float(hi), V::Int(nb as i64 + 1)], vec![])?;
        let edges = as_np(&ev).unwrap().fv();
        let denom = hi - lo;
        let mut counts = vec![0i64; nb];
        for v in &xf {
            if !(*v >= lo && *v <= hi) {
                continue;
            }
            let f = (*v - lo) / denom * nb as f64;
            let mut i = f as i64;
            if i == nb as i64 {
                i -= 1;
            }
            if *v < edges[i as usize] {
                i -= 1;
            }
            if *v >= edges[i as usize + 1] && i != nb as i64 - 1 {
                i += 1;
            }
            counts[i as usize] += 1;
        }
        (counts, edges)
    } else {
        if let V::Str(_) = bins {
            return Err(np_unsup("np.histogram(bins='auto' and other estimators)"));
        }
        let e0 = np_as(&bins)?;
        edge_arr = Some(e0.copy());
        let e = np_float_arr(&bins)?;
        if e.ndim() != 1 {
            return val_err("`bins` must be 1d, when an array".into());
        }
        let edges = e.fv();
        if edges.windows(2).any(|w| w[1] < w[0]) {
            return val_err("`bins` must increase monotonically, when an array".into());
        }
        let mut sa = xf.clone();
        sa.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
        let nb = edges.len().saturating_sub(1);
        let cum: Vec<usize> = edges
            .iter()
            .enumerate()
            .map(|(k, e)| if k + 1 == edges.len() { sa.partition_point(|v| v <= e) } else { sa.partition_point(|v| v < e) })
            .collect();
        ((0..nb).map(|k| (cum[k + 1] - cum[k]) as i64).collect(), edges)
    };
    let ne = edges.len();
    let ev = match edge_arr {
        Some(e) => np_arr(e),
        None => np_new(Buf::F(edges.clone()), vec![ne]),
    };
    if density {
        let tot: i64 = counts.iter().sum();
        let h: Vec<f64> = counts.iter().enumerate().map(|(k, c)| *c as f64 / (edges[k + 1] - edges[k]) / tot as f64).collect();
        return Ok(tup(vec![np_new(Buf::F(h), vec![counts.len()]), ev]));
    }
    let nc = counts.len();
    Ok(tup(vec![np_new(Buf::I(counts), vec![nc]), ev]))
}

// np.cov(m, y=None, rowvar=True, bias=False, ddof=None)
fn np_cov_arr(a: &Args, name: &str) -> Result<NdArr, E> {
    let m0 = npa(a, 0, if name == "cov" { "m" } else { "x" }).ok_or_else(|| exc("TypeError", format!("{name}() missing required argument")))?;
    let rowvar = npa(a, 2, "rowvar").map_or(Ok(true), |v| truth(&v))?;
    let (bias, ddof) = if name == "cov" {
        (npa(a, 3, "bias").map_or(Ok(false), |v| truth(&v))?, npa(a, 4, "ddof").filter(|v| !matches!(v, V::None)))
    } else {
        (false, None)
    };
    for k in ["fweights", "aweights", "dtype"] {
        if a.k.iter().any(|(n, _)| *n == k) {
            return Err(np_unsup(&format!("np.{name}({k}=...)")));
        }
    }
    let prep = |v: &V| -> Result<NdArr, E> {
        let mut x = np_float_arr(v)?;
        if x.ndim() > 2 {
            return val_err("m has more than 2 dimensions".into());
        }
        if x.ndim() < 2 {
            x = x.reshaped(vec![1, x.size()]);
        }
        if !rowvar && x.shape[0] != 1 {
            x = NdArr::new(np_bvals(&np_transpose(&x, None)?, &[x.shape[1], x.shape[0]]), vec![x.shape[1], x.shape[0]]);
        }
        Ok(x.copy())
    };
    let mut x = prep(&m0)?;
    if let Some(y) = npa(a, 1, "y").filter(|v| !matches!(v, V::None)) {
        let y = prep(&y)?;
        x = as_np(&np_concat(&[x, y], Some(0))?).unwrap().clone();
    }
    let (r, n) = (x.shape[0], x.shape[1]);
    let ddof = match ddof {
        Some(d) => np_int_arg(&d)? as f64,
        None => if bias { 0.0 } else { 1.0 },
    };
    let xv = x.fv();
    let mut fact = n as f64 - ddof;
    if fact <= 0.0 {
        np_warn("Degrees of freedom <= 0 for slice");
        fact = 0.0;
    }
    let mut c = vec![0.0; r];
    let rows: Vec<Vec<f64>> = (0..r)
        .map(|i| {
            let row = &xv[i * n..(i + 1) * n];
            let mean = np_pw(row) / n as f64;
            row.iter().map(|v| v - mean).collect()
        })
        .collect();
    c.resize(r * r, 0.0);
    let inv = 1.0 / fact;
    for i in 0..r {
        for j in i..r {
            let d = fchain(&rows[i], &rows[j]) * inv;
            c[i * r + j] = d;
            c[j * r + i] = d;
        }
    }
    Ok(NdArr::new(Buf::F(c), vec![r, r]))
}
pub fn np_cov(a: &Args) -> R {
    let c = np_cov_arr(a, "cov")?;
    if c.size() == 1 {
        // squeezed to a 0-d array, as numpy's
        return Ok(np_arr(NdArr::new(c.vals(), vec![])));
    }
    Ok(np_arr(c))
}
pub fn np_corrcoef(a: &Args) -> R {
    let c = np_cov_arr(a, "corrcoef")?;
    let r = c.shape[0];
    let mut v = c.fv();
    if r == 1 {
        let x = v[0] / v[0];
        return Ok(np_sc(Sc::F(x)));
    }
    let sd: Vec<f64> = (0..r).map(|i| v[i * r + i].sqrt()).collect();
    for i in 0..r {
        for j in 0..r {
            v[i * r + j] /= sd[i];
        }
    }
    for i in 0..r {
        for j in 0..r {
            let x = v[i * r + j] / sd[j];
            v[i * r + j] = if x.is_nan() { x } else { x.clamp(-1.0, 1.0) };
        }
    }
    Ok(np_new(Buf::F(v), vec![r, r]))
}
// np.trapezoid(y, x=None, dx=1.0, axis=-1), for 1-D y
pub fn np_trapezoid(a: &Args) -> R {
    let y0 = npa(a, 0, "y").ok_or_else(|| exc("TypeError", "trapezoid() missing required argument 'y' (pos 0)".into()))?;
    let y = np_as(&y0)?;
    if y.ndim() != 1 || npa(a, 3, "axis").is_some_and(|v| !matches!(v, V::Int(-1) | V::Int(0))) {
        return Err(np_unsup("np.trapezoid of a multi-dimensional array"));
    }
    let yv = np_arr(y);
    let sl = |lo: Option<i64>, hi: Option<i64>| slice(lo.map_or(V::None, V::Int), hi.map_or(V::None, V::Int), V::None);
    let hi = getitem(&yv, &sl(Some(1), None))?;
    let lo = getitem(&yv, &sl(None, Some(-1)))?;
    let d = match npa(a, 1, "x").filter(|v| !matches!(v, V::None)) {
        Some(x) => {
            let xa = np_as(&x)?;
            if xa.ndim() != 1 {
                return Err(np_unsup("np.trapezoid with a multi-dimensional x"));
            }
            np_call("diff", vec![np_arr(xa)], vec![])?
        }
        None => npa(a, 2, "dx").unwrap_or(V::Float(1.0)),
    };
    let s = np_binop(&hi, &lo, "+").unwrap()?;
    let p = np_binop(&d, &s, "*").unwrap()?;
    let q = np_binop(&p, &V::Float(2.0), "/").unwrap()?;
    np_meth(&q, "sum", vec![], vec![])
}

// np.<ufunc>.reduce / .accumulate (axis 0 by default, as numpy's ufunc methods)
pub fn np_ufunc_method(uf: &str, m: &str, a: &Args) -> R {
    let x0 = npa(a, 0, "array").ok_or_else(|| exc("TypeError", format!("{m}() missing required argument 'array' (pos 0)")))?;
    let x = np_as(&x0)?;
    for k in ["dtype", "out", "initial", "where"] {
        if a.k.iter().any(|(n, _)| *n == k) {
            return Err(np_unsup(&format!("np.{uf}.{m}({k}=...)")));
        }
    }
    let axis_v = npa(a, 1, "axis").unwrap_or(V::Int(0));
    if x.ndim() == 0 {
        return val_err(format!("zero-dimensional arrays cannot be {}", if m == "reduce" { "reduced" } else { "accumulated" }));
    }
    let xv = np_arr(x.clone());
    if m == "reduce" {
        let what = match uf {
            "add" => "sum",
            "multiply" => "prod",
            "maximum" => "max",
            "minimum" => "min",
            "logical_and" => "all",
            "logical_or" => "any",
            _ => return Err(np_unsup(&format!("np.{uf}.reduce"))),
        };
        let mut k = vec![("axis", axis_v)];
        if let Some(v) = a.k.iter().find(|(n, _)| *n == "keepdims").map(|(_, v)| v.clone()) {
            k.push(("keepdims", v));
        }
        return np_call(what, vec![xv], k);
    }
    if m != "accumulate" {
        return Err(np_unsup(&format!("np.{uf}.{m}")));
    }
    let ax = match axis_v {
        V::None => return type_err("accumulate does not allow multiple axes".into()),
        v => norm_axis(np_int_arg(&v)?, x.ndim())?,
    };
    match uf {
        "add" => np_accum(&x, "cumsum", Some(ax)),
        "multiply" => np_accum(&x, "cumprod", Some(ax)),
        "maximum" | "minimum" => {
            let mx = uf == "maximum";
            let g = np_groups(&x, Some(ax));
            let n = g.n;
            let ng = if n == 0 { 0 } else { g.vals.len() / n };
            let res = match &g.vals {
                Buf::F(v) => {
                    let mut o = v.clone();
                    for k in 0..ng {
                        for j in 1..n {
                            let (p, q) = (o[k * n + j - 1], v[k * n + j]);
                            o[k * n + j] = if p.is_nan() {
                                p
                            } else if q.is_nan() {
                                q
                            } else if mx {
                                if p > q { p } else { q }
                            } else if p < q {
                                p
                            } else {
                                q
                            };
                        }
                    }
                    Buf::F(o)
                }
                Buf::I(v) => {
                    let mut o = v.clone();
                    for k in 0..ng {
                        for j in 1..n {
                            let (p, q) = (o[k * n + j - 1], v[k * n + j]);
                            o[k * n + j] = if mx { p.max(q) } else { p.min(q) };
                        }
                    }
                    Buf::I(o)
                }
                Buf::B(v) => {
                    let mut o = v.clone();
                    for k in 0..ng {
                        for j in 1..n {
                            let (p, q) = (o[k * n + j - 1], v[k * n + j]);
                            o[k * n + j] = if mx { p || q } else { p && q };
                        }
                    }
                    Buf::B(o)
                }
            };
            np_from_groups(&x, ax, res)
        }
        _ => Err(np_unsup(&format!("np.{uf}.accumulate"))),
    }
}

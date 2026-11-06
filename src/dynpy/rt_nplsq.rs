// ---------- least squares as numpy runs it (np.linalg.lstsq, np.polyfit) ----------
// numpy's lstsq is LAPACK dgelsd from the OpenBLAS numpy ships (0.3.3x, reference LAPACK 3.12 inside).
// LAPACK's own routines are compiled without fused multiply-adds, so they are ported here line by line;
// the BLAS calls they make go through the same kernel models as numpy's matrix products (dgemv_t_4.c,
// dgemv_n_4.c, daxpy.c, the dgemm kernels) plus OpenBLAS's x87 dnrm2 (nrm2.S, which sums squares in
// 80-bit extended precision). Covered: m >= n with n <= 25 columns (dlalsd's direct path, which every
// polyfit up to degree 24 takes). Matrices are column-major, element (i, j) at off + i + j * ld.
const LQ_EPS: f64 = 1.1102230246251565e-16; // dlamch('E') = 2^-53
const LQ_PREC: f64 = 2.220446049250313e-16; // dlamch('P') = 2^-52
const LQ_SFMIN: f64 = 2.2250738585072014e-308; // dlamch('S') = 2^-1022

// x87 extended precision, positive values only: mant * 2^(exp - 63), mant's top bit set (or 0 for zero)
#[derive(Clone, Copy)]
struct X80 {
    m: u64,
    e: i32,
}
fn x80_round(v: u128, e: i32) -> X80 {
    // v (nonzero) * 2^e, rounded to 64 bits, nearest-even
    let lz = v.leading_zeros() as i32;
    let bits = 128 - lz;
    if bits <= 64 {
        let m = (v as u64) << (64 - bits);
        return X80 { m, e: e + bits - 1 };
    }
    let sh = bits - 64;
    let mut m = (v >> sh) as u64;
    let rest = v & ((1u128 << sh) - 1);
    let half = 1u128 << (sh - 1);
    let mut ex = e + bits - 1;
    if rest > half || (rest == half && m & 1 == 1) {
        let (nm, of) = m.overflowing_add(1);
        m = if of {
            ex += 1;
            1u64 << 63
        } else {
            nm
        };
    }
    X80 { m, e: ex }
}
fn x80_sq(x: f64) -> X80 {
    let b = x.abs().to_bits();
    if b == 0 {
        return X80 { m: 0, e: 0 };
    }
    let (fm, fe) = if b >> 52 == 0 { (b & ((1 << 52) - 1), -1074) } else { ((b & ((1 << 52) - 1)) | (1 << 52), ((b >> 52) as i32) - 1075) };
    let p = (fm as u128) * (fm as u128);
    x80_round(p, 2 * fe)
}
fn x80_add(a: X80, b: X80) -> X80 {
    if a.m == 0 {
        return b;
    }
    if b.m == 0 {
        return a;
    }
    let (hi, lo) = if a.e >= b.e { (a, b) } else { (b, a) };
    // value = mant * 2^(e - 63); put hi's mantissa at bit 126 of a u128 (63 guard bits), add lo aligned
    let d = (hi.e - lo.e) as u32;
    let vh = (hi.m as u128) << 63;
    let vl = if d >= 127 {
        1
    } else {
        let full = (lo.m as u128) << 63;
        let s = full >> d;
        if s << d != full { s | 1 } else { s }
    };
    x80_round(vh + vl, hi.e - 126)
}
fn x80_sqrt(a: X80) -> X80 {
    if a.m == 0 {
        return a;
    }
    let e0 = a.e - 63;
    let s = if (e0 - 64).rem_euclid(2) == 0 { 64 } else { 63 };
    let mm = (a.m as u128) << s;
    // integer square root of mm (in [2^126, 2^128)): 64 bits
    let mut r = ((mm as f64).sqrt() as u128).max(1);
    for _ in 0..3 {
        r = (r + mm / r) / 2;
    }
    let sq_gt = |r: u128| r.checked_mul(r).map_or(true, |p| p > mm);
    while sq_gt(r) {
        r -= 1;
    }
    while !sq_gt(r + 1) {
        r += 1;
    }
    let rem = mm - r * r;
    let mut ex = (e0 - s) / 2 + 63;
    let mut m = r as u64;
    if rem > r {
        let (nm, of) = m.overflowing_add(1);
        m = if of {
            ex += 1;
            1u64 << 63
        } else {
            nm
        };
    }
    X80 { m, e: ex }
}
fn x80_f64(a: X80) -> f64 {
    if a.m == 0 {
        return 0.0;
    }
    let mut m = a.m >> 11;
    let rest = a.m & 0x7ff;
    let mut e = a.e;
    if rest > 0x400 || (rest == 0x400 && m & 1 == 1) {
        m += 1;
        if m >> 53 != 0 {
            m >>= 1;
            e += 1;
        }
    }
    // m * 2^(e - 52), scaled in two exact steps when 2^(e - 52) itself is out of range
    let p2 = |k: i32| f64::from_bits(((k + 1023) as u64) << 52);
    let k = e - 52;
    if k < -1022 {
        (m as f64) * p2(k + 600) * p2(-600)
    } else if k > 1023 {
        (m as f64) * p2(k - 600) * p2(600)
    } else {
        (m as f64) * p2(k)
    }
}
// dnrm2 (interface/nrm2.c + kernel/x86_64/nrm2.S): four x87 accumulators, then sqrt in extended precision.
// On x86-64 it runs nrm2.S's own instructions (so the x87 precision setting is the platform's, as numpy's
// is); elsewhere an exact software model of the same steps.
fn lq_nrm2(n: usize, x: &[f64], off: usize, inc: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return x[off].abs();
    }
    assert!(off + (n - 1) * inc < x.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut out: f64 = 0.0;
        std::arch::asm!(
            "fldz", "fldz", "fldz", "fldz",
            "mov {i}, {n}",
            "shr {i}, 3",
            "jz 3f",
            "2:",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "faddp st(7), st(0)", "faddp st(5), st(0)", "faddp st(3), st(0)", "faddp st(1), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)",
            "faddp st(7), st(0)", "faddp st(5), st(0)", "faddp st(3), st(0)", "faddp st(1), st(0)",
            "dec {i}",
            "jnz 2b",
            "3:",
            "and {n}, 7",
            "jz 5f",
            "4:",
            "fld qword ptr [{x}]", "add {x}, {inc}", "fmul st(0), st(0)", "faddp st(1), st(0)",
            "dec {n}",
            "jnz 4b",
            "5:",
            "faddp st(2), st(0)", "faddp st(1), st(0)", "faddp st(1), st(0)",
            "fsqrt",
            "fstp qword ptr [{o}]",
            n = inout(reg) n => _,
            x = inout(reg) x.as_ptr().add(off) => _,
            inc = in(reg) inc * 8,
            i = out(reg) _,
            o = in(reg) &mut out as *mut f64,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack)
        );
        return out;
    }
    #[allow(unreachable_code)]
    lq_nrm2_soft(n, x, off, inc)
}
fn lq_nrm2_soft(n: usize, x: &[f64], off: usize, inc: usize) -> f64 {
    let mut acc = [X80 { m: 0, e: 0 }; 4];
    let at = |i: usize| x[off + i * inc];
    let n8 = n / 8 * 8;
    for i in 0..n8 {
        acc[i % 4] = x80_add(acc[i % 4], x80_sq(at(i)));
    }
    for i in n8..n {
        acc[0] = x80_add(acc[0], x80_sq(at(i)));
    }
    let s = x80_add(x80_add(x80_add(acc[2], acc[0]), acc[1]), acc[3]);
    x80_f64(x80_sqrt(s))
}
fn lq_scal(n: usize, da: f64, x: &mut [f64], off: usize, inc: usize) {
    if da == 1.0 {
        return;
    }
    for i in 0..n {
        x[off + i * inc] *= da;
    }
}
// daxpy.c: y += da * x; the vector part (and every element on SkylakeX) fused
fma_on!(lq_axpy, lq_axpy_i, (k: &Lk, n: usize, da: f64, x: &[f64], xo: usize, incx: usize, y: &mut [f64], yo: usize, incy: usize) -> ());
#[inline(always)]
fn lq_axpy_i(k: &Lk, n: usize, da: f64, x: &[f64], xo: usize, incx: usize, y: &mut [f64], yo: usize, incy: usize) {
    if n == 0 || da == 0.0 {
        return;
    }
    if incx == 1 && incy == 1 {
        let n1 = if k.skx { n } else { n & !15 };
        let (xs, ys) = (&x[xo..xo + n], &mut y[yo..yo + n]);
        for i in 0..n1 {
            ys[i] = fma(da, xs[i], ys[i]);
        }
        for i in n1..n {
            ys[i] += da * xs[i];
        }
        return;
    }
    for i in 0..n {
        let (xv, yi) = (x[xo + i * incx], yo + i * incy);
        y[yi] = if k.skx { fma(da, xv, y[yi]) } else { y[yi] + da * xv };
    }
}
// dger (interface/ger.c + kernel/generic/ger.c): A += alpha x y^T, one daxpy per column
fn lq_ger(k: &Lk, m: usize, n: usize, alpha: f64, x: &[f64], y: &[f64], a: &mut [f64], ao: usize, lda: usize) {
    if m == 0 || n == 0 || alpha == 0.0 {
        return;
    }
    for j in 0..n {
        lq_axpy(k, m, alpha * y[j], x, 0, 1, a, ao + j * lda, 1);
    }
}
// drot (kernel/x86_64/drot.c), strided: x' = c x + s y, y' = c y - s x
fn lq_rot(_k: &Lk, n: usize, a: &mut [f64], xo: usize, yo: usize, inc: usize, c: f64, s: f64) {
    for i in 0..n {
        let (xi, yi) = (xo + i * inc, yo + i * inc);
        let (x, y) = (a[xi], a[yi]);
        // fused on both kernels (drot.c is built with FMA for Haswell too)
        a[xi] = fma(c, x, s * y);
        a[yi] = fma(c, y, -(s * x));
    }
}
fn lq_swap(n: usize, a: &mut [f64], xo: usize, yo: usize, inc: usize) {
    for i in 0..n {
        a.swap(xo + i * inc, yo + i * inc);
    }
}
fn lq_sign(a: f64, b: f64) -> f64 {
    a.abs().copysign(b)
}
fn lq_lapy2(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return if y.is_nan() { y } else { x };
    }
    let (xa, ya) = (x.abs(), y.abs());
    let w = xa.max(ya);
    let z = xa.min(ya);
    if z == 0.0 || w > f64::MAX {
        w
    } else {
        w * (1.0 + (z / w) * (z / w)).sqrt()
    }
}
// dlarfg: the Householder reflector for (alpha, x); alpha at a[ai], x at a[xo..] step inc; returns tau
fn lq_larfg(n: usize, a: &mut [f64], ai: usize, xo: usize, inc: usize) -> f64 {
    if n <= 1 {
        return 0.0;
    }
    let mut xnorm = lq_nrm2(n - 1, a, xo, inc);
    if xnorm == 0.0 {
        return 0.0;
    }
    let mut alpha = a[ai];
    let mut beta = -lq_sign(lq_lapy2(alpha, xnorm), alpha);
    let safmin = LQ_SFMIN / LQ_EPS;
    let mut knt = 0;
    if beta.abs() < safmin {
        let rsafmn = 1.0 / safmin;
        loop {
            knt += 1;
            lq_scal(n - 1, rsafmn, a, xo, inc);
            beta *= rsafmn;
            alpha *= rsafmn;
            if !(beta.abs() < safmin && knt < 20) {
                break;
            }
        }
        xnorm = lq_nrm2(n - 1, a, xo, inc);
        beta = -lq_sign(lq_lapy2(alpha, xnorm), alpha);
    }
    let tau = (beta - alpha) / beta;
    lq_scal(n - 1, 1.0 / (alpha - beta), a, xo, inc);
    for _ in 0..knt {
        beta *= safmin;
    }
    a[ai] = beta;
    tau
}
// dgemv 'T', alpha 1, beta 0: w(j) = sum_t C(t, j) v(t) for the m x n block of c at co (ld ldc)
fn lq_gemv_t(m: usize, n: usize, c: &[f64], co: usize, ldc: usize, v: &[f64]) -> Vec<f64> {
    if m == 0 || n == 0 {
        return vec![0.0; n];
    }
    if np_blas() == Blas::Plain {
        return (0..n).map(|j| dot_f(&c[co + j * ldc..co + j * ldc + m], v)).collect();
    }
    np_gemv_t_ld(c, co, ldc, n, m, v)
}
// dgemv 'N', alpha 1, beta 0: w(i) = sum_t C(i, t) v(t); `unit` whether v was contiguous
fn lq_gemv_n(m: usize, n: usize, c: &[f64], co: usize, ldc: usize, v: &[f64], unit: bool) -> Vec<f64> {
    if m == 0 || n == 0 {
        return vec![0.0; m];
    }
    let mut a = Vec::with_capacity(m * n);
    for j in 0..n {
        a.extend_from_slice(&c[co + j * ldc..co + j * ldc + m]);
    }
    if np_blas() == Blas::Plain {
        return (0..m).map(|i| (0..n).fold(0.0, |s, t| s + a[i + t * m] * v[t])).collect();
    }
    np_gemv_n_ld(&a, m, n, v, unit, ldc == m)
}
fn lq_iladlc(m: usize, n: usize, c: &[f64], co: usize, ldc: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if c[co + (n - 1) * ldc] != 0.0 || c[co + m - 1 + (n - 1) * ldc] != 0.0 {
        return n;
    }
    for j in (1..=n).rev() {
        for i in 0..m {
            if c[co + i + (j - 1) * ldc] != 0.0 {
                return j;
            }
        }
    }
    0
}
fn lq_iladlr(m: usize, n: usize, c: &[f64], co: usize, ldc: usize) -> usize {
    if m == 0 {
        return 0;
    }
    if c[co + m - 1] != 0.0 || c[co + m - 1 + (n - 1) * ldc] != 0.0 {
        return m;
    }
    let mut r = 0;
    for j in 0..n {
        let mut i = m;
        while i >= 1 && c[co + i - 1 + j * ldc] == 0.0 {
            i -= 1;
        }
        r = r.max(i);
    }
    r
}
// dlarf1f: apply H = I - tau v v^T (v(1) = 1, not stored; v[1..] holds the rest, read at step incv)
// from the left (C is m x n) or the right
fn lq_larf1f(k: &Lk, left: bool, m: usize, n: usize, vfull: &[f64], incv: usize, tau: f64, c: &mut [f64], co: usize, ldc: usize) {
    let mut lastv = 1;
    let mut lastc = 0;
    if tau != 0.0 {
        lastv = if left { m } else { n };
        while lastv > 1 && vfull[lastv - 1] == 0.0 {
            lastv -= 1;
        }
        lastc = if left { lq_iladlc(lastv, n, c, co, ldc) } else { lq_iladlr(m, lastv, c, co, ldc) };
    }
    if lastc == 0 {
        return;
    }
    let v = &vfull[1..lastv];
    if left {
        if lastv == 1 {
            lq_scal(lastc, 1.0 - tau, c, co, ldc);
            return;
        }
        let mut w = lq_gemv_t(lastv - 1, lastc, c, co + 1, ldc, v);
        let row: Vec<f64> = (0..lastc).map(|j| c[co + j * ldc]).collect();
        lq_axpy(k, lastc, 1.0, &row, 0, 1, &mut w, 0, 1);
        lq_axpy(k, lastc, -tau, &w, 0, 1, c, co, ldc);
        lq_ger(k, lastv - 1, lastc, -tau, v, &w, c, co + 1, ldc);
    } else {
        if lastv == 1 {
            lq_scal(lastc, 1.0 - tau, c, co, 1);
            return;
        }
        let mut w = lq_gemv_n(lastc, lastv - 1, c, co + ldc, ldc, v, incv == 1);
        lq_axpy(k, lastc, 1.0, &c[co..co + lastc].to_vec(), 0, 1, &mut w, 0, 1);
        lq_axpy(k, lastc, -tau, &w, 0, 1, c, co, 1);
        lq_ger(k, lastc, lastv - 1, -tau, &w, v, c, co + ldc, ldc);
    }
}
// the reflector vector stored in a (from ai, step inc, n values) as a dense copy
fn lq_vec(a: &[f64], ai: usize, inc: usize, n: usize) -> Vec<f64> {
    (0..n).map(|i| a[ai + i * inc]).collect()
}
// dgeqr2 (dgeqrf takes it for fewer than 32 columns)
fn lq_geqr2(k: &Lk, m: usize, n: usize, a: &mut [f64], lda: usize) -> Vec<f64> {
    let kk = m.min(n);
    let mut tau = vec![0.0; kk];
    for i in 0..kk {
        tau[i] = lq_larfg(m - i, a, i + i * lda, (i + 1).min(m - 1) + i * lda, 1);
        if i + 1 < n {
            let v = lq_vec(a, i + i * lda, 1, m - i);
            lq_larf1f(k, true, m - i, n - i - 1, &v, 1, tau[i], a, i + (i + 1) * lda, lda);
        }
    }
    tau
}
// dorm2r 'L', 'T': C := Q^T C with column reflectors stored in a from ao (as dgeqr2 / dgebd2 leave them)
fn lq_orm2r_lt(k: &Lk, m: usize, n: usize, kk: usize, a: &[f64], ao: usize, lda: usize, tau: &[f64], c: &mut [f64], co: usize, ldc: usize) {
    if m == 0 || n == 0 || kk == 0 {
        return;
    }
    for i in 0..kk {
        let v = lq_vec(a, ao + i + i * lda, 1, m - i);
        lq_larf1f(k, true, m - i, n, &v, 1, tau[i], c, co + i, ldc);
    }
}
// dgelq2 (dgelqf takes it for fewer than 32 rows)
fn lq_gelq2(k: &Lk, m: usize, n: usize, a: &mut [f64], lda: usize) -> Vec<f64> {
    let kk = m.min(n);
    let mut tau = vec![0.0; kk];
    for i in 0..kk {
        tau[i] = lq_larfg(n - i, a, i + i * lda, i + (i + 1).min(n - 1) * lda, lda);
        if i + 1 < m {
            let v = lq_vec(a, i + i * lda, lda, n - i);
            lq_larf1f(k, false, m - i - 1, n - i, &v, lda, tau[i], a, i + 1 + i * lda, lda);
        }
    }
    tau
}
// dgebd2: A = Q B P^T with B upper bidiagonal (m >= n) or lower bidiagonal (m < n): (d, e, tauq, taup)
fn lq_gebd2(k: &Lk, m: usize, n: usize, a: &mut [f64], lda: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let mn = m.min(n);
    let (mut d, mut e, mut tq, mut tp) = (vec![0.0; mn], vec![0.0; mn.saturating_sub(1)], vec![0.0; mn], vec![0.0; mn]);
    if m >= n {
        for i in 0..n {
            tq[i] = lq_larfg(m - i, a, i + i * lda, (i + 1).min(m - 1) + i * lda, 1);
            d[i] = a[i + i * lda];
            if i + 1 < n {
                let v = lq_vec(a, i + i * lda, 1, m - i);
                lq_larf1f(k, true, m - i, n - i - 1, &v, 1, tq[i], a, i + (i + 1) * lda, lda);
                tp[i] = lq_larfg(n - i - 1, a, i + (i + 1) * lda, i + (i + 2).min(n - 1) * lda, lda);
                e[i] = a[i + (i + 1) * lda];
                let v = lq_vec(a, i + (i + 1) * lda, lda, n - i - 1);
                lq_larf1f(k, false, m - i - 1, n - i - 1, &v, lda, tp[i], a, i + 1 + (i + 1) * lda, lda);
            } else {
                tp[i] = 0.0;
            }
        }
    } else {
        for i in 0..m {
            tp[i] = lq_larfg(n - i, a, i + i * lda, i + (i + 1).min(n - 1) * lda, lda);
            d[i] = a[i + i * lda];
            if i + 1 < m {
                let v = lq_vec(a, i + i * lda, lda, n - i);
                lq_larf1f(k, false, m - i - 1, n - i, &v, lda, tp[i], a, i + 1 + i * lda, lda);
                tq[i] = lq_larfg(m - i - 1, a, i + 1 + i * lda, (i + 2).min(m - 1) + i * lda, 1);
                e[i] = a[i + 1 + i * lda];
                let v = lq_vec(a, i + 1 + i * lda, 1, m - i - 1);
                lq_larf1f(k, true, m - i - 1, n - i - 1, &v, 1, tq[i], a, i + 1 + (i + 1) * lda, lda);
            } else {
                tq[i] = 0.0;
            }
        }
    }
    (d, e, tq, tp)
}
fn lq_lange(m: usize, n: usize, a: &[f64], lda: usize) -> f64 {
    let mut v: f64 = 0.0;
    for j in 0..n {
        for i in 0..m {
            let t = a[i + j * lda].abs();
            if v < t || t.is_nan() {
                v = t;
            }
        }
    }
    v
}
// dlascl 'G': multiply the m x n block at off by cto / cfrom without over/underflow
fn lq_lascl(cfrom: f64, cto: f64, m: usize, n: usize, a: &mut [f64], off: usize, lda: usize) {
    if cfrom == 0.0 || cfrom.is_nan() || cto.is_nan() || m == 0 || n == 0 {
        return;
    }
    let smlnum = LQ_SFMIN;
    let bignum = 1.0 / smlnum;
    let (mut cfromc, mut ctoc) = (cfrom, cto);
    loop {
        let cfrom1 = cfromc * smlnum;
        let mul;
        let done;
        if cfrom1 == cfromc {
            mul = ctoc / cfromc;
            done = true;
        } else {
            let cto1 = ctoc / bignum;
            if cto1 == ctoc {
                mul = ctoc;
                done = true;
                cfromc = 1.0;
            } else if cfrom1.abs() > ctoc.abs() && ctoc != 0.0 {
                mul = smlnum;
                done = false;
                cfromc = cfrom1;
            } else if cto1.abs() > cfromc.abs() {
                mul = bignum;
                done = false;
                ctoc = cto1;
            } else {
                mul = ctoc / cfromc;
                done = true;
                if mul == 1.0 {
                    return;
                }
            }
        }
        for j in 0..n {
            for i in 0..m {
                a[off + i + j * lda] *= mul;
            }
        }
        if done {
            return;
        }
    }
}
// dlartg (LAPACK 3.10+ f90 version)
fn lq_lartg(f: f64, g: f64) -> (f64, f64, f64) {
    let safmin = LQ_SFMIN;
    let safmax = 1.0 / safmin;
    let rtmin = safmin.sqrt();
    let rtmax = (safmax / 2.0).sqrt();
    let (f1, g1) = (f.abs(), g.abs());
    if g == 0.0 {
        (1.0, 0.0, f)
    } else if f == 0.0 {
        (0.0, lq_sign(1.0, g), g1)
    } else if f1 > rtmin && f1 < rtmax && g1 > rtmin && g1 < rtmax {
        let d = (f * f + g * g).sqrt();
        let c = f1 / d;
        let r = lq_sign(d, f);
        (c, g / r, r)
    } else {
        let u = safmax.min(safmin.max(f1).max(g1));
        let (fs, gs) = (f / u, g / u);
        let d = (fs * fs + gs * gs).sqrt();
        let c = fs.abs() / d;
        let r = lq_sign(d, f);
        (c, gs / r, r * u)
    }
}
// dlas2: singular values of [[f, g], [0, h]] (ssmin, ssmax)
fn lq_las2(f: f64, g: f64, h: f64) -> (f64, f64) {
    let (fa, ga, ha) = (f.abs(), g.abs(), h.abs());
    let fhmn = fa.min(ha);
    let fhmx = fa.max(ha);
    if fhmn == 0.0 {
        let ssmax = if fhmx == 0.0 {
            ga
        } else {
            let (mx, mn) = (fhmx.max(ga), fhmx.min(ga));
            mx * (1.0 + (mn / mx) * (mn / mx)).sqrt()
        };
        (0.0, ssmax)
    } else if ga < fhmx {
        let as_ = 1.0 + fhmn / fhmx;
        let at = (fhmx - fhmn) / fhmx;
        let au = (ga / fhmx) * (ga / fhmx);
        let c = 2.0 / ((as_ * as_ + au).sqrt() + (at * at + au).sqrt());
        (fhmn * c, fhmx / c)
    } else {
        let au = fhmx / ga;
        if au == 0.0 {
            ((fhmn * fhmx) / ga, ga)
        } else {
            let as_ = 1.0 + fhmn / fhmx;
            let at = (fhmx - fhmn) / fhmx;
            let c = 1.0 / ((1.0 + (as_ * au) * (as_ * au)).sqrt() + (1.0 + (at * au) * (at * au)).sqrt());
            let ssmin = (fhmn * c) * au;
            (ssmin + ssmin, ga / (c + c))
        }
    }
}
// dlasv2: SVD of [[f, g], [0, h]]: (ssmin, ssmax, snr, csr, snl, csl)
fn lq_lasv2(f: f64, g: f64, h: f64) -> (f64, f64, f64, f64, f64, f64) {
    let (mut ft, mut fa, mut ht, mut ha) = (f, f.abs(), h, h.abs());
    let mut pmax = 1;
    let swap = ha > fa;
    if swap {
        pmax = 3;
        std::mem::swap(&mut ft, &mut ht);
        std::mem::swap(&mut fa, &mut ha);
    }
    let gt = g;
    let ga = gt.abs();
    let (mut ssmin, mut ssmax, mut clt, mut crt, mut slt, mut srt) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    if ga == 0.0 {
        ssmin = ha;
        ssmax = fa;
        clt = 1.0;
        crt = 1.0;
        slt = 0.0;
        srt = 0.0;
    } else {
        let mut gasmal = true;
        if ga > fa {
            pmax = 2;
            if fa / ga < LQ_EPS {
                gasmal = false;
                ssmax = ga;
                ssmin = if ha > 1.0 { fa / (ga / ha) } else { (fa / ga) * ha };
                clt = 1.0;
                slt = ht / gt;
                srt = 1.0;
                crt = ft / gt;
            }
        }
        if gasmal {
            let d = fa - ha;
            let mut l = if d == fa { 1.0 } else { d / fa };
            let m = gt / ft;
            let mut t = 2.0 - l;
            let mm = m * m;
            let tt = t * t;
            let s = (tt + mm).sqrt();
            let r = if l == 0.0 { m.abs() } else { (l * l + mm).sqrt() };
            let a = 0.5 * (s + r);
            ssmin = ha / a;
            ssmax = fa * a;
            if mm == 0.0 {
                t = if l == 0.0 { lq_sign(2.0, ft) * lq_sign(1.0, gt) } else { gt / lq_sign(d, ft) + m / t };
            } else {
                t = (m / (s + t) + m / (r + l)) * (1.0 + a);
            }
            l = (t * t + 4.0).sqrt();
            crt = 2.0 / l;
            srt = t / l;
            clt = (crt + srt * m) / a;
            slt = (ht / ft) * srt / a;
        }
    }
    let (csl, snl, csr, snr) = if swap { (srt, crt, slt, clt) } else { (clt, slt, crt, srt) };
    let tsign = match pmax {
        1 => lq_sign(1.0, csr) * lq_sign(1.0, csl) * lq_sign(1.0, f),
        2 => lq_sign(1.0, snr) * lq_sign(1.0, csl) * lq_sign(1.0, g),
        _ => lq_sign(1.0, snr) * lq_sign(1.0, snl) * lq_sign(1.0, h),
    };
    ssmax = lq_sign(ssmax, tsign);
    ssmin = lq_sign(ssmin, tsign * lq_sign(1.0, f) * lq_sign(1.0, h));
    (ssmin, ssmax, snr, csr, snl, csl)
}
// dlasr 'L', 'V', direction forward or backward: rotations of rows (j, j+1) of the m x n block at off
fn lq_lasr_lv(fwd: bool, m: usize, n: usize, c: &[f64], s: &[f64], a: &mut [f64], off: usize, lda: usize) {
    if m == 0 || n == 0 {
        return;
    }
    let mut step = |j: usize| {
        let (ct, st) = (c[j], s[j]);
        if ct != 1.0 || st != 0.0 {
            for i in 0..n {
                let p = off + j + i * lda;
                let temp = a[p + 1];
                a[p + 1] = ct * temp - st * a[p];
                a[p] = st * temp + ct * a[p];
            }
        }
    };
    if fwd {
        for j in 0..m - 1 {
            step(j);
        }
    } else {
        for j in (0..m - 1).rev() {
            step(j);
        }
    }
}
// dbdsqr 'U' with nru = 0: SVD of the upper bidiagonal (d, e), rotations applied to VT (n x ncvt)
// and C (n x ncc). Returns LAPACK's info (> 0: did not converge).
fn lq_bdsqr(k: &Lk, n: usize, ncvt: usize, ncc: usize, d: &mut [f64], e: &mut [f64], vt: &mut [f64], ldvt: usize, c: &mut [f64], ldc: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut info = 0;
    if n > 1 {
        let nm1 = n - 1;
        let (nm12, nm13) = (nm1 + nm1, nm1 + nm1 + nm1);
        let mut work = vec![0.0; 4 * n];
        let eps = LQ_EPS;
        let unfl = LQ_SFMIN;
        let tolmul = 10f64.max(100f64.min(LQ_EPS_M18));
        let tol = tolmul * eps;
        let mut smax: f64 = 0.0;
        for i in 0..n {
            smax = smax.max(d[i].abs());
        }
        for i in 0..n - 1 {
            smax = smax.max(e[i].abs());
        }
        let mut smin: f64 = 0.0;
        let thresh;
        {
            let mut sminoa = d[0].abs();
            if sminoa != 0.0 {
                let mut mu = sminoa;
                for i in 1..n {
                    mu = d[i].abs() * (mu / (mu + e[i - 1].abs()));
                    sminoa = sminoa.min(mu);
                    if sminoa == 0.0 {
                        break;
                    }
                }
            }
            sminoa /= (n as f64).sqrt();
            thresh = (tol * sminoa).max(6.0 * (n as f64 * (n as f64 * unfl)));
        }
        let maxitdivn = 6 * n;
        let (mut iterdivn, mut iter): (usize, isize) = (0, -1);
        let (mut oldll, mut oldm): (isize, isize) = (-1, -1);
        let mut idir = 0;
        // 1-based indices below, as in LAPACK
        let mut m = n;
        macro_rules! D {
            ($i:expr) => {
                d[$i - 1]
            };
        }
        macro_rules! E {
            ($i:expr) => {
                e[$i - 1]
            };
        }
        'main: loop {
            if m <= 1 {
                break;
            }
            if iter >= n as isize {
                iter -= n as isize;
                iterdivn += 1;
                if iterdivn >= maxitdivn {
                    info = 0;
                    for i in 0..n - 1 {
                        if e[i] != 0.0 {
                            info += 1;
                        }
                    }
                    return info;
                }
            }
            smax = D!(m).abs();
            let mut ll = 0;
            let mut split = false;
            for lll in 1..m {
                ll = m - lll;
                let abss = D!(ll).abs();
                let abse = E!(ll).abs();
                if abse <= thresh {
                    split = true;
                    break;
                }
                smax = smax.max(abss).max(abse);
            }
            if split {
                E!(ll) = 0.0;
                if ll == m - 1 {
                    m -= 1;
                    continue 'main;
                }
            } else {
                ll = 0;
            }
            ll += 1;
            if ll == m - 1 {
                let (sigmn, sigmx, sinr, cosr, sinl, cosl) = lq_lasv2(D!(m - 1), E!(m - 1), D!(m));
                D!(m - 1) = sigmx;
                E!(m - 1) = 0.0;
                D!(m) = sigmn;
                if ncvt > 0 {
                    lq_rot(k, ncvt, vt, m - 2, m - 1, ldvt, cosr, sinr);
                }
                if ncc > 0 {
                    lq_rot(k, ncc, c, m - 2, m - 1, ldc, cosl, sinl);
                }
                m -= 2;
                continue 'main;
            }
            if ll as isize > oldm || (m as isize) < oldll {
                idir = if D!(ll).abs() >= D!(m).abs() { 1 } else { 2 };
            }
            if idir == 1 {
                if E!(m - 1).abs() <= tol.abs() * D!(m).abs() {
                    E!(m - 1) = 0.0;
                    continue 'main;
                }
                let mut mu = D!(ll).abs();
                smin = mu;
                for lll in ll..m {
                    if E!(lll).abs() <= tol * mu {
                        E!(lll) = 0.0;
                        continue 'main;
                    }
                    mu = D!(lll + 1).abs() * (mu / (mu + E!(lll).abs()));
                    smin = smin.min(mu);
                }
            } else {
                if E!(ll).abs() <= tol.abs() * D!(ll).abs() {
                    E!(ll) = 0.0;
                    continue 'main;
                }
                let mut mu = D!(m).abs();
                smin = mu;
                for lll in (ll..m).rev() {
                    if E!(lll).abs() <= tol * mu {
                        E!(lll) = 0.0;
                        continue 'main;
                    }
                    mu = D!(lll).abs() * (mu / (mu + E!(lll).abs()));
                    smin = smin.min(mu);
                }
            }
            oldll = ll as isize;
            oldm = m as isize;
            let mut shift;
            if n as f64 * tol * (smin / smax) <= eps.max(0.01 * tol) {
                shift = 0.0;
            } else {
                let sll;
                if idir == 1 {
                    sll = D!(ll).abs();
                    shift = lq_las2(D!(m - 1), E!(m - 1), D!(m)).0;
                } else {
                    sll = D!(m).abs();
                    shift = lq_las2(D!(ll), E!(ll), D!(ll + 1)).0;
                }
                if sll > 0.0 && (shift / sll) * (shift / sll) < eps {
                    shift = 0.0;
                }
            }
            iter += (m - ll) as isize;
            let cnt = m - ll + 1;
            if shift == 0.0 {
                if idir == 1 {
                    let (mut cs, mut oldcs) = (1.0, 1.0);
                    let (mut sn, mut oldsn) = (0.0, 0.0);
                    for i in ll..m {
                        let (c1, s1, r) = lq_lartg(D!(i) * cs, E!(i));
                        cs = c1;
                        sn = s1;
                        if i > ll {
                            E!(i - 1) = oldsn * r;
                        }
                        let (c2, s2, r2) = lq_lartg(oldcs * r, D!(i + 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        D!(i) = r2;
                        work[i - ll] = cs;
                        work[i - ll + nm1] = sn;
                        work[i - ll + nm12] = oldcs;
                        work[i - ll + nm13] = oldsn;
                    }
                    let h = D!(m) * cs;
                    D!(m) = h * oldcs;
                    E!(m - 1) = h * oldsn;
                    if ncvt > 0 {
                        lq_lasr_lv(true, cnt, ncvt, &work[0..], &work[nm1..], vt, ll - 1, ldvt);
                    }
                    if ncc > 0 {
                        lq_lasr_lv(true, cnt, ncc, &work[nm12..], &work[nm13..], c, ll - 1, ldc);
                    }
                    if E!(m - 1).abs() <= thresh {
                        E!(m - 1) = 0.0;
                    }
                } else {
                    let (mut cs, mut oldcs) = (1.0, 1.0);
                    let (mut sn, mut oldsn) = (0.0, 0.0);
                    for i in (ll + 1..=m).rev() {
                        let (c1, s1, r) = lq_lartg(D!(i) * cs, E!(i - 1));
                        cs = c1;
                        sn = s1;
                        if i < m {
                            E!(i) = oldsn * r;
                        }
                        let (c2, s2, r2) = lq_lartg(oldcs * r, D!(i - 1) * sn);
                        oldcs = c2;
                        oldsn = s2;
                        D!(i) = r2;
                        work[i - ll - 1] = cs;
                        work[i - ll - 1 + nm1] = -sn;
                        work[i - ll - 1 + nm12] = oldcs;
                        work[i - ll - 1 + nm13] = -oldsn;
                    }
                    let h = D!(ll) * cs;
                    D!(ll) = h * oldcs;
                    E!(ll) = h * oldsn;
                    if ncvt > 0 {
                        lq_lasr_lv(false, cnt, ncvt, &work[nm12..], &work[nm13..], vt, ll - 1, ldvt);
                    }
                    if ncc > 0 {
                        lq_lasr_lv(false, cnt, ncc, &work[0..], &work[nm1..], c, ll - 1, ldc);
                    }
                    if E!(ll).abs() <= thresh {
                        E!(ll) = 0.0;
                    }
                }
            } else if idir == 1 {
                let mut f = (D!(ll).abs() - shift) * (lq_sign(1.0, D!(ll)) + shift / D!(ll));
                let mut g = E!(ll);
                for i in ll..m {
                    let (cosr, sinr, r) = lq_lartg(f, g);
                    if i > ll {
                        E!(i - 1) = r;
                    }
                    f = cosr * D!(i) + sinr * E!(i);
                    E!(i) = cosr * E!(i) - sinr * D!(i);
                    g = sinr * D!(i + 1);
                    D!(i + 1) = cosr * D!(i + 1);
                    let (cosl, sinl, r) = lq_lartg(f, g);
                    D!(i) = r;
                    f = cosl * E!(i) + sinl * D!(i + 1);
                    D!(i + 1) = cosl * D!(i + 1) - sinl * E!(i);
                    if i < m - 1 {
                        g = sinl * E!(i + 1);
                        E!(i + 1) = cosl * E!(i + 1);
                    }
                    work[i - ll] = cosr;
                    work[i - ll + nm1] = sinr;
                    work[i - ll + nm12] = cosl;
                    work[i - ll + nm13] = sinl;
                }
                E!(m - 1) = f;
                if ncvt > 0 {
                    lq_lasr_lv(true, cnt, ncvt, &work[0..], &work[nm1..], vt, ll - 1, ldvt);
                }
                if ncc > 0 {
                    lq_lasr_lv(true, cnt, ncc, &work[nm12..], &work[nm13..], c, ll - 1, ldc);
                }
                if E!(m - 1).abs() <= thresh {
                    E!(m - 1) = 0.0;
                }
            } else {
                let mut f = (D!(m).abs() - shift) * (lq_sign(1.0, D!(m)) + shift / D!(m));
                let mut g = E!(m - 1);
                for i in (ll + 1..=m).rev() {
                    let (cosr, sinr, r) = lq_lartg(f, g);
                    if i < m {
                        E!(i) = r;
                    }
                    f = cosr * D!(i) + sinr * E!(i - 1);
                    E!(i - 1) = cosr * E!(i - 1) - sinr * D!(i);
                    g = sinr * D!(i - 1);
                    D!(i - 1) = cosr * D!(i - 1);
                    let (cosl, sinl, r) = lq_lartg(f, g);
                    D!(i) = r;
                    f = cosl * E!(i - 1) + sinl * D!(i - 1);
                    D!(i - 1) = cosl * D!(i - 1) - sinl * E!(i - 1);
                    if i > ll + 1 {
                        g = sinl * E!(i - 2);
                        E!(i - 2) = cosl * E!(i - 2);
                    }
                    work[i - ll - 1] = cosr;
                    work[i - ll - 1 + nm1] = -sinr;
                    work[i - ll - 1 + nm12] = cosl;
                    work[i - ll - 1 + nm13] = -sinl;
                }
                E!(ll) = f;
                if E!(ll).abs() <= thresh {
                    E!(ll) = 0.0;
                }
                if ncvt > 0 {
                    lq_lasr_lv(false, cnt, ncvt, &work[nm12..], &work[nm13..], vt, ll - 1, ldvt);
                }
                if ncc > 0 {
                    lq_lasr_lv(false, cnt, ncc, &work[0..], &work[nm1..], c, ll - 1, ldc);
                }
            }
        }
    }
    // make the singular values positive, then sort them into decreasing order
    for i in 0..n {
        if d[i] == 0.0 {
            d[i] = 0.0;
        }
        if d[i] < 0.0 {
            d[i] = -d[i];
            if ncvt > 0 {
                lq_scal(ncvt, -1.0, vt, i, ldvt);
            }
        }
    }
    for i in 1..n {
        let mut isub = 1;
        let mut smin = d[0];
        for j in 2..=n + 1 - i {
            if d[j - 1] <= smin {
                isub = j;
                smin = d[j - 1];
            }
        }
        let last = n + 1 - i;
        if isub != last {
            d[isub - 1] = d[last - 1];
            d[last - 1] = smin;
            if ncvt > 0 {
                lq_swap(ncvt, vt, isub - 1, last - 1, ldvt);
            }
            if ncc > 0 {
                lq_swap(ncc, c, isub - 1, last - 1, ldc);
            }
        }
    }
    info
}
// eps^(-1/8) for dbdsqr's TOLMUL (pow(2^-53, -0.125) as glibc rounds it)
const LQ_EPS_M18: f64 = 98.70496038052;
// dlalsd 'U' for n <= 25 (SMLSIZ): the minimum-norm solution of the bidiagonal problem, into b
fn lq_lalsd(k: &Lk, lower: bool, n: usize, nrhs: usize, d: &mut [f64], e: &mut [f64], b: &mut [f64], ldb: usize, rcond: f64) -> Result<usize, usize> {
    let rcnd = if rcond <= 0.0 || rcond >= 1.0 { LQ_EPS } else { rcond };
    let mut rank = 0;
    if n == 0 {
        return Ok(0);
    }
    if n == 1 {
        if d[0] == 0.0 {
            for j in 0..nrhs {
                b[j * ldb] = 0.0;
            }
        } else {
            rank = 1;
            lq_lascl(d[0], 1.0, 1, nrhs, b, 0, ldb);
            d[0] = d[0].abs();
        }
        return Ok(rank);
    }
    let nm1 = n - 1;
    if lower {
        // rotate the lower bidiagonal to upper, applying the rotations to b
        let mut work = vec![0.0; 2 * n];
        for i in 0..n - 1 {
            let (cs, sn, r) = lq_lartg(d[i], e[i]);
            d[i] = r;
            e[i] = sn * d[i + 1];
            d[i + 1] = cs * d[i + 1];
            if nrhs == 1 {
                lq_rot(k, 1, b, i, i + 1, 1, cs, sn);
            } else {
                work[2 * i] = cs;
                work[2 * i + 1] = sn;
            }
        }
        if nrhs > 1 {
            for c in 0..nrhs {
                for j in 0..n - 1 {
                    lq_rot(k, 1, b, j + c * ldb, j + 1 + c * ldb, 1, work[2 * j], work[2 * j + 1]);
                }
            }
        }
    }
    // dlanst('M')
    let mut orgnrm = d[n - 1].abs();
    for i in 0..n - 1 {
        let s = d[i].abs();
        if orgnrm < s || s.is_nan() {
            orgnrm = s;
        }
        let s = e[i].abs();
        if orgnrm < s || s.is_nan() {
            orgnrm = s;
        }
    }
    if orgnrm == 0.0 {
        for j in 0..nrhs {
            for i in 0..n {
                b[i + j * ldb] = 0.0;
            }
        }
        return Ok(0);
    }
    lq_lascl(orgnrm, 1.0, n, 1, d, 0, n);
    lq_lascl(orgnrm, 1.0, nm1, 1, e, 0, nm1);
    let mut w = vec![0.0; n * n];
    for i in 0..n {
        w[i + i * n] = 1.0;
    }
    // dlasdq('U', sqre 0, ncvt n, nru 0, ncc nrhs) = dbdsqr, then its sort into increasing order
    let info = lq_bdsqr(k, n, n, nrhs, d, e, &mut w, n, b, ldb);
    if info != 0 {
        return Err(info);
    }
    for i in 0..n {
        let mut isub = i;
        let mut smin = d[i];
        for j in i + 1..n {
            if d[j] < smin {
                isub = j;
                smin = d[j];
            }
        }
        if isub != i {
            d[isub] = d[i];
            d[i] = smin;
            lq_swap(n, &mut w, isub, i, n);
            lq_swap(nrhs, b, isub, i, ldb);
        }
    }
    // idamax: the first largest |d|
    let mut im = 0;
    for i in 1..n {
        if d[i].abs() > d[im].abs() {
            im = i;
        }
    }
    let tol = rcnd * d[im].abs();
    for i in 0..n {
        if d[i] <= tol {
            for j in 0..nrhs {
                b[i + j * ldb] = 0.0;
            }
        } else {
            lq_lascl(d[i], 1.0, 1, nrhs, b, i, ldb);
            rank += 1;
        }
    }
    // dgemm('T', 'N', n, nrhs, n, 1, work, n, b, ldb, 0, ...)
    let xv: Vec<f64> = (0..nrhs).flat_map(|j| b[j * ldb..j * ldb + n].to_vec()).collect();
    let prod = if np_blas() == Blas::Plain {
        let mut c = vec![0.0; n * nrhs];
        for j in 0..nrhs {
            for i in 0..n {
                c[j * n + i] = dot_f(&w[i * n..(i + 1) * n], &xv[j * n..(j + 1) * n]);
            }
        }
        c
    } else {
        let yv: Vec<f64> = (0..n * n).map(|q| w[q / n + (q % n) * n]).collect();
        np_gemm(&xv, &yv, nrhs, n, n, false, true)
    };
    for j in 0..nrhs {
        for i in 0..n {
            b[i + j * ldb] = prod[j * n + i];
        }
    }
    lq_lascl(1.0, orgnrm, n, 1, d, 0, n);
    d.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    lq_lascl(orgnrm, 1.0, n, nrhs, b, 0, ldb);
    Ok(rank)
}
// dgelsd for min(m, n) <= 25: a is m x n (lda m), b is max(m, n) x nrhs (ldb max(m, n)), overwritten with
// the solution in its first n rows. Returns (singular values, rank) or Err(info) when the SVD did not converge.
fn lq_gelsd(m: usize, n: usize, nrhs: usize, a: &mut [f64], b: &mut [f64], rcond: f64) -> Result<(Vec<f64>, usize), usize> {
    let k = lk();
    let (lda, ldb) = (m, m.max(n));
    let minmn = m.min(n);
    let mut s = vec![0.0; minmn];
    if m == 0 || n == 0 {
        return Ok((s, 0));
    }
    let eps = LQ_PREC;
    let sfmin = LQ_SFMIN;
    let smlnum = sfmin / eps;
    let bignum = 1.0 / smlnum;
    let anrm = lq_lange(m, n, a, lda);
    let mut iascl = 0u8;
    if anrm > 0.0 && anrm < smlnum {
        lq_lascl(anrm, smlnum, m, n, a, 0, lda);
        iascl = 1;
    } else if anrm > bignum {
        lq_lascl(anrm, bignum, m, n, a, 0, lda);
        iascl = 2;
    } else if anrm == 0.0 {
        for v in b.iter_mut() {
            *v = 0.0;
        }
        return Ok((s, 0));
    }
    let bnrm = lq_lange(m, nrhs, b, ldb);
    let mut ibscl = 0u8;
    if bnrm > 0.0 && bnrm < smlnum {
        lq_lascl(bnrm, smlnum, m, nrhs, b, 0, ldb);
        ibscl = 1;
    } else if bnrm > bignum {
        lq_lascl(bnrm, bignum, m, nrhs, b, 0, ldb);
        ibscl = 2;
    }
    if m < n {
        for j in 0..nrhs {
            for i in m..n {
                b[i + j * ldb] = 0.0;
            }
        }
    }
    // MNTHR = ILAENV(6): INT(REAL(MIN(M, N)) * 1.6E0), in single precision
    let mnthr = ((minmn as f32) * 1.6f32) as usize;
    if m >= n {
        let mut mm = m;
        if m >= mnthr {
            mm = n;
            let tau = lq_geqr2(&k, m, n, a, lda);
            lq_orm2r_lt(&k, m, nrhs, n, a, 0, lda, &tau, b, 0, ldb);
            for j in 0..n {
                for i in j + 1..n {
                    a[i + j * lda] = 0.0;
                }
            }
        }
        let (mut d, mut e, tq, tp) = lq_gebd2(&k, mm, n, a, lda);
        // dormbr('Q', 'L', 'T') is dormqr on the column reflectors
        lq_orm2r_lt(&k, mm, nrhs, n, a, 0, lda, &tq, b, 0, ldb);
        let rank = lq_lalsd(&k, false, n, nrhs, &mut d, &mut e, b, ldb, rcond)?;
        // dormbr('P', 'L', 'N', n, nrhs, n): dormlq('L', 'T', n - 1, ...) on A(1, 2), C(2, 1)
        if n > 1 {
            lq_orml2_ln_t(&k, n - 1, nrhs, n - 1, a, lda, lda, &tp, b, 1, ldb);
        }
        s.copy_from_slice(&d);
        lq_unscale(iascl, ibscl, anrm, bnrm, smlnum, bignum, n, nrhs, b, ldb, &mut s);
        return Ok((s, rank));
    }
    let rank;
    if n >= mnthr {
        // an LQ factorization first, then the m x m problem
        let tau = lq_gelq2(&k, m, n, a, lda);
        let mut w = vec![0.0; m * m];
        for j in 0..m {
            for i in j..m {
                w[i + j * m] = a[i + j * lda];
            }
        }
        let (mut d, mut e, tq, tp) = lq_gebd2(&k, m, m, &mut w, m);
        lq_orm2r_lt(&k, m, nrhs, m, &w, 0, m, &tq, b, 0, ldb);
        rank = lq_lalsd(&k, false, m, nrhs, &mut d, &mut e, b, ldb, rcond)?;
        if m > 1 {
            lq_orml2_ln_t(&k, m - 1, nrhs, m - 1, &w, m, m, &tp, b, 1, ldb);
        }
        for j in 0..nrhs {
            for i in m..n {
                b[i + j * ldb] = 0.0;
            }
        }
        lq_orml2_ln_t(&k, n, nrhs, m, a, 0, lda, &tau, b, 0, ldb);
        s.copy_from_slice(&d);
    } else {
        let (mut d, mut e, tq, tp) = lq_gebd2(&k, m, n, a, lda);
        // dormbr('Q', 'L', 'T', m, nrhs, n): nq = m < k = n, so dormqr on A(2, 1), C(2, 1)
        if m > 1 {
            lq_orm2r_lt(&k, m - 1, nrhs, m - 1, a, 1, lda, &tq, b, 1, ldb);
        }
        rank = lq_lalsd(&k, true, m, nrhs, &mut d, &mut e, b, ldb, rcond)?;
        // dormbr('P', 'L', 'N', n, nrhs, m): nq = n > k = m, so dormlq on A itself
        lq_orml2_ln_t(&k, n, nrhs, m, a, 0, lda, &tp, b, 0, ldb);
        s.copy_from_slice(&d);
    }
    lq_unscale(iascl, ibscl, anrm, bnrm, smlnum, bignum, n, nrhs, b, ldb, &mut s);
    Ok((s, rank))
}
fn lq_unscale(iascl: u8, ibscl: u8, anrm: f64, bnrm: f64, smlnum: f64, bignum: f64, n: usize, nrhs: usize, b: &mut [f64], ldb: usize, s: &mut [f64]) {
    let mn = s.len();
    if iascl == 1 {
        lq_lascl(anrm, smlnum, n, nrhs, b, 0, ldb);
        lq_lascl(smlnum, anrm, mn, 1, s, 0, mn);
    } else if iascl == 2 {
        lq_lascl(anrm, bignum, n, nrhs, b, 0, ldb);
        lq_lascl(bignum, anrm, mn, 1, s, 0, mn);
    }
    if ibscl == 1 {
        lq_lascl(smlnum, bnrm, n, nrhs, b, 0, ldb);
    } else if ibscl == 2 {
        lq_lascl(bignum, bnrm, n, nrhs, b, 0, ldb);
    }
}
// dorml2('L', 'T'): C := P^T... with the row reflectors (dormbr 'P', 'N' passes TRANST = 'T')
fn lq_orml2_ln_t(k: &Lk, m: usize, n: usize, kk: usize, a: &[f64], ao: usize, lda: usize, tau: &[f64], c: &mut [f64], co: usize, ldc: usize) {
    if m == 0 || n == 0 || kk == 0 {
        return;
    }
    // LEFT and TRANS = 'T': i from k down to 1
    for i in (0..kk).rev() {
        let v = lq_vec(a, ao + i + i * lda, lda, m - i);
        lq_larf1f(k, true, m - i, n, &v, lda, tau[i], c, co + i, ldc);
    }
}

// ---------- np.linalg.lstsq, np.polyfit, np.polyval ----------
// numpy's lstsq for float a (m x n) and b (m x nrhs), both C order: (x n x nrhs C order, residuals when
// numpy returns them, rank, singular values)
fn np_lstsq_parts(av: &[f64], m: usize, n: usize, bv: &[f64], nrhs: usize, rcond: f64) -> Result<(Vec<f64>, Option<Vec<f64>>, usize, Vec<f64>), E> {
    if m == 0 || n == 0 || nrhs == 0 {
        return Err(np_unsup("np.linalg.lstsq of an empty matrix"));
    }
    if m.min(n) > 25 {
        return Err(np_unsup("np.linalg.lstsq of a matrix with more than 25 rows and columns"));
    }
    let mut a = lk_colmajor(av, m, n);
    let ldb = m.max(n);
    let mut b = vec![0.0; ldb * nrhs];
    for i in 0..m {
        for j in 0..nrhs {
            b[i + j * ldb] = bv[i * nrhs + j];
        }
    }
    let (s, rank) = lq_gelsd(m, n, nrhs, &mut a, &mut b, rcond).map_err(|_| linalg_err("SVD did not converge in Linear Least Squares"))?;
    let mut x = vec![0.0; n * nrhs];
    for i in 0..n {
        for j in 0..nrhs {
            x[i * nrhs + j] = b[i + j * ldb];
        }
    }
    // umath_linalg's abs2 over each column's last m - n rows, when the rank is full and m > n
    let resid = if rank == n && m > n {
        Some(
            (0..nrhs)
                .map(|j| {
                    let mut r = 0.0;
                    for i in n..m {
                        let e = b[i + j * ldb];
                        r += e * e;
                    }
                    r
                })
                .collect(),
        )
    } else {
        None
    };
    Ok((x, resid, rank, s))
}
fn lq_2d(v: &V, what: &str) -> Result<NdArr, E> {
    let a = np_float_arr(v)?;
    if a.ndim() != 2 {
        if a.ndim() < 2 {
            return Err(linalg_err(&format!("{}-dimensional array given. Array must be two-dimensional", a.ndim())));
        }
        return Err(np_unsup(&format!("{what} on stacks of matrices")));
    }
    Ok(a)
}
fn lq_rcond(v: Option<V>, dflt: f64) -> Result<f64, E> {
    match v {
        None | Some(V::None) => Ok(dflt),
        Some(r) => np_f_arg(&r),
    }
}
pub fn np_lstsq(a: &Args) -> R {
    npa_kw_ok(a, "lstsq", &["a", "b", "rcond"])?;
    let av = npa(a, 0, "a").ok_or_else(|| exc("TypeError", "lstsq() missing required argument 'a' (pos 0)".into()))?;
    let bv = npa(a, 1, "b").ok_or_else(|| exc("TypeError", "lstsq() missing required argument 'b' (pos 1)".into()))?;
    let am = lq_2d(&av, "np.linalg.lstsq")?;
    let bm = np_float_arr(&bv)?;
    let is1 = bm.ndim() == 1;
    if bm.ndim() == 0 {
        return Err(linalg_err("0-dimensional array given. Array must be at least two-dimensional"));
    }
    if bm.ndim() > 2 {
        return Err(np_unsup("np.linalg.lstsq on stacks of matrices"));
    }
    let (m, n) = (am.shape[0], am.shape[1]);
    let (m2, nrhs) = if is1 { (bm.shape[0], 1) } else { (bm.shape[0], bm.shape[1]) };
    if m != m2 {
        return Err(linalg_err("Incompatible dimensions"));
    }
    let rcond = lq_rcond(npa(a, 2, "rcond"), LQ_PREC * m.max(n) as f64)?;
    let (x, resid, rank, s) = np_lstsq_parts(&am.fv(), m, n, &bm.fv(), nrhs, rcond)?;
    let xs = if is1 { vec![n] } else { vec![n, nrhs] };
    let r = match resid {
        Some(r) => np_new(Buf::F(r), vec![nrhs]),
        None => np_new(Buf::F(vec![]), vec![0]),
    };
    let ns = s.len();
    Ok(tup(vec![np_new(Buf::F(x), xs), r, np_sc(Sc::I32(rank as i32)), np_new(Buf::F(s), vec![ns])]))
}
thread_local! {
    static LQ_WARNED: Cell<bool> = Cell::new(false);
}
pub fn np_polyfit(a: &Args) -> R {
    npa_kw_ok(a, "polyfit", &["x", "y", "deg", "rcond", "full", "w", "cov"])?;
    let need = |i: usize, nm: &str| npa(a, i, nm).ok_or_else(|| exc("TypeError", format!("polyfit() missing required argument '{nm}' (pos {i})")));
    let (x0, y0, d0) = (need(0, "x")?, need(1, "y")?, need(2, "deg")?);
    let deg = to_int(&d0)?;
    let deg = match deg {
        V::Int(d) => d,
        _ => return Err(np_unsup("np.polyfit with a huge degree")),
    };
    // x = asarray(x) + 0.0 (which also turns -0.0 into 0.0)
    let plus0 = |a: NdArr| NdArr::new(Buf::F(a.fv().iter().map(|v| v + 0.0).collect()), a.shape.clone());
    let x = plus0(np_float_arr(&x0)?);
    let y = plus0(np_float_arr(&y0)?);
    if deg < 0 {
        return val_err("expected deg >= 0".into());
    }
    if x.ndim() != 1 {
        return type_err("expected 1D vector for x".into());
    }
    if x.size() == 0 {
        return type_err("expected non-empty vector for x".into());
    }
    if y.ndim() < 1 || y.ndim() > 2 {
        return type_err("expected 1D or 2D array for y".into());
    }
    if x.shape[0] != y.shape[0] {
        return type_err("expected x and y to have same length".into());
    }
    let order = deg as usize + 1;
    let m = x.shape[0];
    let rcond = lq_rcond(npa(a, 3, "rcond"), m as f64 * LQ_PREC)?;
    let full = npa(a, 4, "full").map_or(Ok(false), |v| truth(&v))?;
    let covv = npa(a, 6, "cov").unwrap_or(V::Bool(false));
    let xv = x.fv();
    // vander(x, order): x^k by repeated multiplies, highest power first
    let mut lhs = vec![0.0; m * order];
    for i in 0..m {
        let mut p = 1.0;
        lhs[i * order + order - 1] = 1.0;
        for k in 1..order {
            p = if k == 1 { xv[i] } else { p * xv[i] };
            lhs[i * order + order - 1 - k] = p;
        }
    }
    let mut lhs_v = np_new(Buf::F(lhs), vec![m, order]);
    let mut rhs_v = np_arr(y.clone());
    if let Some(w) = npa(a, 5, "w").filter(|v| !matches!(v, V::None)) {
        let w = np_float_arr(&w)?;
        if w.ndim() != 1 {
            return type_err("expected a 1-d array for weights".into());
        }
        if w.shape[0] != y.shape[0] {
            return type_err("expected w and y to have the same length".into());
        }
        let wcol = np_new(Buf::F(w.fv()), vec![m, 1]);
        lhs_v = np_binop(&lhs_v, &wcol, "*").unwrap()?;
        rhs_v = if y.ndim() == 2 { np_binop(&rhs_v, &wcol, "*").unwrap()? } else { np_binop(&rhs_v, &np_arr(w), "*").unwrap()? };
    }
    // scale = sqrt((lhs * lhs).sum(axis=0)); lhs /= scale
    let sq = np_binop(&lhs_v, &lhs_v, "*").unwrap()?;
    let ss = np_meth(&sq, "sum", vec![], vec![(sym("axis"), V::Int(0))])?;
    let scale = np_call("sqrt", vec![ss], vec![])?;
    let lhs_v = np_binop(&lhs_v, &scale, "/").unwrap()?;
    let lhs_a = as_np(&lhs_v).unwrap().clone();
    let rhs_a = as_np(&rhs_v).unwrap().clone();
    let nrhs = if rhs_a.ndim() == 2 { rhs_a.shape[1] } else { 1 };
    let (c, resid, rank, s) = np_lstsq_parts(&lhs_a.fv(), m, order, &rhs_a.fv(), nrhs, rcond)?;
    let scv = as_np(&scale).unwrap().fv();
    let mut cv = c;
    for i in 0..order {
        for j in 0..nrhs {
            cv[i * nrhs + j] /= scv[i];
        }
    }
    let cshape = if rhs_a.ndim() == 2 { vec![order, nrhs] } else { vec![order] };
    let c = np_new(Buf::F(cv), cshape);
    if rank != order && !full && !LQ_WARNED.with(|w| w.replace(true)) {
        out_flush();
        err_write("RankWarning: Polyfit may be poorly conditioned\n");
    }
    let resids = match resid {
        Some(r) => np_new(Buf::F(r), vec![nrhs]),
        None => np_new(Buf::F(vec![]), vec![0]),
    };
    if full {
        // rcond as numpy has it: len(x) * finfo.eps is a numpy float64, a given one stays as given
        let rc = match npa(a, 3, "rcond").filter(|v| !matches!(v, V::None)) {
            Some(v) => v,
            None => np_sc(Sc::F(rcond)),
        };
        let ns = s.len();
        return Ok(tup(vec![c, resids, np_sc(Sc::I32(rank as i32)), np_new(Buf::F(s), vec![ns]), rc]));
    }
    let unscaled = match &covv {
        V::Str(_) => {
            if eq(&covv, &st("unscaled".into()))? {
                true
            } else {
                false
            }
        }
        _ => {
            if !truth(&covv)? {
                return Ok(c);
            }
            false
        }
    };
    // Vbase = inv(dot(lhs.T, lhs)) / outer(scale, scale)
    let lt = np_meth(&lhs_v, "transpose", vec![], vec![])?;
    let g = np_call("dot", vec![lt, lhs_v.clone()], vec![])?;
    let vb = np_linalg_call("inv", &mut Args::p(vec![g]))?;
    let o = np_call("outer", vec![scale.clone(), scale.clone()], vec![])?;
    let vb = np_binop(&vb, &o, "/").unwrap()?;
    let fac = if unscaled {
        V::Int(1)
    } else {
        if m <= order {
            return val_err("the number of data points must exceed order to scale the covariance matrix".into());
        }
        np_binop(&resids, &V::Int((m - order) as i64), "/").unwrap()?
    };
    let cov = if y.ndim() == 1 {
        np_binop(&vb, &fac, "*").unwrap()?
    } else {
        let vb3 = np_meth(&vb, "reshape", vec![V::Int(order as i64), V::Int(order as i64), V::Int(1)], vec![])?;
        np_binop(&vb3, &fac, "*").unwrap()?
    };
    Ok(tup(vec![c, cov]))
}
// np.polyval(p, x): Horner's rule with numpy's own multiply and add
pub fn np_polyval(a: &Args) -> R {
    npa_kw_ok(a, "polyval", &["p", "x"])?;
    let p0 = npa(a, 0, "p").ok_or_else(|| exc("TypeError", "polyval() missing required argument 'p' (pos 0)".into()))?;
    let x0 = npa(a, 1, "x").ok_or_else(|| exc("TypeError", "polyval() missing required argument 'x' (pos 1)".into()))?;
    let p = np_arr(np_as(&p0)?);
    let x = np_arr(np_as(&x0)?);
    let mut y = np_call("zeros_like", vec![x.clone()], vec![])?;
    for pv in to_vec(&p)? {
        y = np_binop(&np_binop(&y, &x, "*").unwrap()?, &pv, "+").unwrap()?;
    }
    Ok(y)
}

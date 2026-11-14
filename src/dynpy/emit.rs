// Emits Rust for a parsed Python program, against the dynamic runtime (rt_*.rs).
use super::parse::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

type ER<T> = Result<T, (usize, String)>;

pub const BUILTIN_MODULES: [&str; 52] = ["_brevi_col", "_brevi_srv", "_brevi_proc", "importlib.metadata", "zoneinfo", "tzlocal", "atexit", "_brevi_async", "_brevi_net", "_brevi_fs", "_brevi_st", "_brevi_ort", "_brevi_sqlite", "_brevi_duckdb", "_brevi_stdio", "_brevi_tok", "_brevi_hf", "_brevi_mt", "_brevi_term", "hashlib", "base64", "binascii", "json.decoder", "enum", "sys", "math", "collections", "heapq", "bisect", "functools", "itertools", "copy", "operator", "string", "time", "typing", "dataclasses", "abc", "re", "re._constants", "re._compiler", "json", "__future__", "decimal", "fractions", "datetime", "csv", "html", "statistics", "threading", "gc", "weakref"];


// ---------- numpy, natively (rt_np*.rs) ----------
// Set while a build uses Brevi's own numpy (on until a refusal names numpy; then CPython's numpy is used).
pub static NP_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
// Set while the program being emitted imports numpy natively.
pub static NP_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// The program uses Brevi's pydantic: defs carry their number of positional parameters (src/dynpy/pyd.rs).
pub static PYD_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// With Brevi's pydantic, per module (pyd::rewrite): the model annotations' lines, which their descriptor
// statement evaluates in place of the annotation, and the names holding descriptors or the pydantic copy, which
// are not CPython's typing values (an annotation reading one is refused where CPython evaluates it).
pub static PYD_ANN: std::sync::Mutex<Option<HashMap<String, (HashSet<usize>, HashSet<String>)>>> = std::sync::Mutex::new(None);
// Set while the program is a .bv file in Brevi's own spellings, turned into Python (main.rs bv_as_py): its
// annotations are Brevi's types, which may name a class defined further down, so none is evaluated.
pub static BV_SPELLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn np_native() -> bool {
    NP_NATIVE.load(std::sync::atomic::Ordering::Relaxed)
}
fn np_on() -> bool {
    NP_ON.load(std::sync::atomic::Ordering::Relaxed)
}
// Set while the program names float32 or int32 (so its arrays may be of those types): only the numpy functions
// in NP_NARROW_FUNCS are native then, and `@` is refused (float32 matmul is OpenBLAS's sgemm, not modelled).
pub static NP_NARROW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn np_narrow() -> bool {
    NP_NARROW.load(std::sync::atomic::Ordering::Relaxed)
}
// Whether Python source names the float32 or int32 dtypes.
pub fn names_narrow(src: &str) -> bool {
    if ["float32", "int32", "intc", "single"].iter().any(|t| src.contains(t)) {
        return true;
    }
    for q in ['\'', '"'] {
        for t in ["f4", "i4", "<f4", "<i4", "f", "i"] {
            let lit = format!("{q}{t}{q}");
            if t.len() > 1 && src.contains(&lit) {
                return true;
            }
            // a one-letter code only as a dtype: dtype='f', astype('i'), np.dtype("f")
            for pre in ["dtype=", "dtype = ", "astype(", "dtype("] {
                if src.contains(&format!("{pre}{lit}")) {
                    return true;
                }
            }
        }
    }
    false
}
// The numpy functions native for float32 and int32 arrays (the runtime's np_narrow_ok has the same list).
// float32 ufuncs whose numpy SIMD loops the runtime reproduces, for numpy's AVX-512 (SkylakeX) dispatch only
// (rt_npfn.rs np_exp_f32 and the rest). On another CPU numpy runs other code, so the program goes to CPython.
const NP_F32_SIMD: &[&str] = &["exp", "tanh", "log1p", "sin", "cos"];
fn np_f32_simd_here() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::is_x86_feature_detected as has;
        return has!("avx512f") && has!("avx512dq") && has!("avx512cd") && has!("avx512bw") && has!("avx512vl");
    }
    #[allow(unreachable_code)]
    false
}
const NP_NARROW_FUNCS: &[&str] = &[
    "pi", "e", "inf", "nan", "euler_gamma", "newaxis", "True_", "False_", "float64", "double", "int64", "int_", "intp", "bool_", "bool", "ndarray",
    "integer", "signedinteger", "floating", "number", "generic", "inexact", "float32", "single", "int32", "intc",
    "array", "asarray", "asanyarray", "copy", "zeros", "ones", "empty", "full", "zeros_like", "ones_like", "empty_like", "full_like",
    "arange", "linspace", "eye", "identity", "set_printoptions", "shape", "ndim", "size", "dtype", "isscalar", "issubdtype",
    "sum", "mean", "std", "var", "prod", "max", "min", "amax", "amin", "argmax", "argmin", "any", "all", "cumsum", "cumprod", "round", "around", "clip",
    "reshape", "ravel", "transpose", "swapaxes", "squeeze", "diagonal", "nonzero", "argsort", "repeat", "sort", "count_nonzero", "str_", "where", "argwhere", "flatnonzero",
    "unique", "concatenate", "stack", "vstack", "hstack", "column_stack", "atleast_1d", "atleast_2d", "expand_dims", "flip", "fliplr", "flipud", "roll",
    "diff", "outer", "array_equal", "searchsorted", "tile", "meshgrid", "diag", "fill_diagonal", "unravel_index",
    "abs", "absolute", "negative", "positive", "sqrt", "floor", "ceil", "trunc", "rint", "fix", "square", "reciprocal", "sign", "isnan", "isinf", "isfinite", "logical_not",
    "add", "subtract", "multiply", "divide", "true_divide", "floor_divide", "mod", "remainder", "power", "maximum", "minimum", "fmax", "fmin",
    "equal", "not_equal", "less", "less_equal", "greater", "greater_equal", "logical_and", "logical_or", "logical_xor",
    "bitwise_and", "bitwise_or", "bitwise_xor", "left_shift", "right_shift", "random",
];
// false: `string` is CPython's Lib/string.py compiled in (py/string.py), for Template and Formatter; true (the
// default): the native module, constants only, which builds and starts much faster
pub static STRING_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
// inspect is Brevi's own while the program uses only its Signature (Signature.from_callable, rt_sig.rs); a program
// using anything else of inspect is built again with CPython's inspect through the bridge.
pub static INSPECT_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn builtin_mod(m: &str) -> bool {
    (m == "inspect" && INSPECT_NATIVE.load(std::sync::atomic::Ordering::Relaxed)) || (BUILTIN_MODULES.contains(&m) && (m != "string" || STRING_NATIVE.load(std::sync::atomic::Ordering::Relaxed))) || m == "typing_extensions" || (matches!(m, "numpy" | "numpy.random" | "numpy.linalg") && np_native())
}
// The numpy functions and attributes the native numpy has, with the keyword arguments each takes.
const NP_FUNCS: &[(&str, &[&str])] = &[
    ("pi", &[]), ("e", &[]), ("inf", &[]), ("nan", &[]), ("euler_gamma", &[]), ("newaxis", &[]), ("True_", &[]), ("False_", &[]),
    ("float64", &[]), ("double", &[]), ("int64", &[]), ("int_", &[]), ("intp", &[]), ("bool_", &[]), ("bool", &[]), ("ndarray", &[]),
    ("float32", &[]), ("single", &[]), ("int32", &[]), ("intc", &[]), ("str_", &[]),
    ("integer", &[]), ("signedinteger", &[]), ("floating", &[]), ("number", &[]), ("generic", &[]), ("inexact", &[]),
    ("array", &["dtype", "copy", "ndmin"]), ("asarray", &["dtype"]), ("asanyarray", &["dtype"]), ("copy", &[]),
    ("zeros", &["dtype"]), ("ones", &["dtype"]), ("empty", &["dtype"]), ("full", &["dtype", "fill_value"]),
    ("zeros_like", &["dtype"]), ("ones_like", &["dtype"]), ("empty_like", &["dtype"]), ("full_like", &["dtype", "fill_value"]),
    ("arange", &["start", "stop", "step", "dtype"]), ("linspace", &["num", "endpoint", "retstep", "dtype"]), ("eye", &["M", "k", "dtype"]), ("identity", &["dtype"]),
    ("set_printoptions", &["precision", "suppress", "linewidth", "threshold", "edgeitems"]),
    ("shape", &[]), ("ndim", &[]), ("size", &[]), ("dtype", &[]), ("isscalar", &[]), ("issubdtype", &[]),
    ("sum", &["axis", "keepdims"]), ("mean", &["axis", "keepdims"]), ("std", &["axis", "keepdims", "ddof"]), ("var", &["axis", "keepdims", "ddof"]),
    ("prod", &["axis", "keepdims"]), ("max", &["axis", "keepdims"]), ("min", &["axis", "keepdims"]), ("amax", &["axis", "keepdims"]), ("amin", &["axis", "keepdims"]),
    ("argmax", &["axis", "keepdims"]), ("argmin", &["axis", "keepdims"]), ("any", &["axis", "keepdims"]), ("all", &["axis", "keepdims"]),
    ("cumsum", &["axis"]), ("cumprod", &["axis"]), ("round", &["decimals"]), ("around", &["decimals"]), ("clip", &["a_min", "a_max", "min", "max"]),
    ("reshape", &["shape", "newshape"]), ("ravel", &[]), ("transpose", &["axes"]), ("swapaxes", &["axis1", "axis2"]), ("squeeze", &["axis"]), ("trace", &[]), ("diagonal", &["offset"]),
    ("nonzero", &[]), ("argsort", &["axis", "kind"]), ("repeat", &["axis"]), ("sort", &["axis", "kind"]), ("median", &["axis", "keepdims"]),
    ("percentile", &["axis", "keepdims", "method"]), ("quantile", &["axis", "keepdims", "method"]),
    ("nansum", &["axis", "keepdims"]), ("nanmean", &["axis", "keepdims"]), ("nanstd", &["axis", "keepdims", "ddof"]), ("nanvar", &["axis", "keepdims", "ddof"]), ("nanmax", &["axis", "keepdims"]), ("nanmin", &["axis", "keepdims"]),
    ("average", &["axis", "weights"]), ("count_nonzero", &["axis"]), ("where", &[]), ("argwhere", &[]), ("flatnonzero", &[]),
    ("unique", &["return_index", "return_inverse", "return_counts"]), ("concatenate", &["axis"]), ("stack", &["axis"]), ("vstack", &[]), ("hstack", &[]), ("column_stack", &[]),
    ("atleast_1d", &[]), ("atleast_2d", &[]), ("expand_dims", &["axis"]), ("flip", &["axis"]), ("fliplr", &[]), ("flipud", &[]), ("roll", &["shift", "axis"]),
    ("diff", &["n", "axis", "prepend", "append"]), ("outer", &[]), ("dot", &[]), ("matmul", &[]), ("inner", &[]), ("vdot", &[]),
    ("isclose", &["rtol", "atol", "equal_nan"]), ("allclose", &["rtol", "atol", "equal_nan"]), ("array_equal", &[]), ("searchsorted", &["side"]),
    ("tile", &[]), ("meshgrid", &["indexing"]), ("diag", &["k"]), ("fill_diagonal", &[]), ("unravel_index", &[]), ("cross", &[]), ("convolve", &["mode"]),
    ("histogram", &["bins", "range", "density", "weights"]), ("cov", &["m", "y", "rowvar", "bias", "ddof"]), ("corrcoef", &["x", "y", "rowvar"]), ("trapezoid", &["y", "x", "dx", "axis"]), ("polyfit", &["x", "y", "deg", "rcond", "full", "w", "cov"]), ("polyval", &["p", "x"]),
    ("abs", &[]), ("absolute", &[]), ("fabs", &[]), ("negative", &[]), ("positive", &[]), ("sqrt", &[]), ("exp", &[]), ("exp2", &[]), ("expm1", &[]), ("log", &[]), ("log10", &[]), ("log2", &[]), ("log1p", &[]),
    ("sin", &[]), ("cos", &[]), ("tan", &[]), ("arcsin", &[]), ("arccos", &[]), ("arctan", &[]), ("sinh", &[]), ("cosh", &[]), ("tanh", &[]), ("arcsinh", &[]), ("arccosh", &[]), ("arctanh", &[]),
    ("cbrt", &[]), ("floor", &[]), ("ceil", &[]), ("trunc", &[]), ("rint", &[]), ("fix", &[]), ("square", &[]), ("reciprocal", &[]), ("sign", &[]), ("degrees", &[]), ("radians", &[]), ("deg2rad", &[]), ("rad2deg", &[]),
    ("isnan", &[]), ("isinf", &[]), ("isfinite", &[]), ("logical_not", &[]),
    ("add", &[]), ("subtract", &[]), ("multiply", &[]), ("divide", &[]), ("true_divide", &[]), ("floor_divide", &[]), ("mod", &[]), ("remainder", &[]), ("power", &[]),
    ("maximum", &[]), ("minimum", &[]), ("fmax", &[]), ("fmin", &[]), ("arctan2", &[]), ("hypot", &[]), ("copysign", &[]), ("fmod", &[]), ("logaddexp", &[]),
    ("equal", &[]), ("not_equal", &[]), ("less", &[]), ("less_equal", &[]), ("greater", &[]), ("greater_equal", &[]),
    ("logical_and", &[]), ("logical_or", &[]), ("logical_xor", &[]), ("bitwise_and", &[]), ("bitwise_or", &[]), ("bitwise_xor", &[]), ("left_shift", &[]), ("right_shift", &[]),
];
// Every attribute name of numpy arrays, scalars and dtypes; the second list is those the native numpy has
// (with the keyword arguments of the methods). A program using any other of these names gets CPython's numpy.
const NP_ATTR_ALL: &[&str] = &["T", "alignment", "all", "any", "argmax", "argmin", "argpartition", "argsort", "as_integer_ratio", "astype", "base", "bit_count", "byteorder", "byteswap", "char", "choose", "clip", "compress", "conj", "conjugate", "copy", "ctypes", "cumprod", "cumsum", "data", "denominator", "descr", "device", "diagonal", "dot", "dtype", "dump", "dumps", "fields", "fill", "flags", "flat", "flatten", "fromhex", "getfield", "hasobject", "hex", "imag", "is_integer", "isalignedstruct", "isbuiltin", "isnative", "item", "itemsize", "kind", "mT", "max", "mean", "metadata", "min", "name", "names", "nbytes", "ndim", "newbyteorder", "nonzero", "num", "numerator", "partition", "prod", "ptp", "put", "ravel", "real", "repeat", "reshape", "resize", "round", "searchsorted", "setfield", "setflags", "shape", "size", "sort", "squeeze", "std", "str", "strides", "subdtype", "sum", "swapaxes", "take", "to_device", "tobytes", "tofile", "tolist", "trace", "transpose", "type", "var", "view", "integers", "standard_normal", "exponential", "standard_exponential", "permutation", "binomial", "poisson", "beta", "gamma", "lognormal", "multivariate_normal", "geometric", "laplace", "logistic", "chisquare", "dirichlet", "gumbel", "hypergeometric", "multinomial", "multivariate_hypergeometric", "negative_binomial", "noncentral_chisquare", "noncentral_f", "pareto", "power", "rayleigh", "standard_cauchy", "standard_gamma", "standard_t", "vonmises", "wald", "zipf", "permuted", "spawn", "bit_generator", "random_integers", "tomaxint", "get_state", "set_state", "rand", "randn", "random_sample", "ranf", "randint", "normal", "uniform", "choice", "shuffle", "sample", "random", "seed", "accumulate", "reduce", "reduceat", "at", "nin", "nout", "nargs", "ntypes", "types", "signature"];
const NP_ATTR_OK: &[(&str, &[&str])] = &[
    ("T", &[]), ("mT", &[]), ("shape", &[]), ("ndim", &[]), ("size", &[]), ("dtype", &[]), ("itemsize", &[]), ("nbytes", &[]), ("real", &[]), ("strides", &[]), ("imag", &[]),
    ("name", &[]), ("kind", &[]), ("char", &[]), ("type", &[]),
    ("sum", &["axis", "keepdims"]), ("mean", &["axis", "keepdims"]), ("std", &["axis", "keepdims", "ddof"]), ("var", &["axis", "keepdims", "ddof"]), ("prod", &["axis", "keepdims"]),
    ("max", &["axis", "keepdims"]), ("min", &["axis", "keepdims"]), ("argmax", &["axis", "keepdims"]), ("argmin", &["axis", "keepdims"]), ("any", &["axis", "keepdims"]), ("all", &["axis", "keepdims"]),
    ("cumsum", &["axis"]), ("cumprod", &["axis"]), ("round", &["decimals"]), ("clip", &["min", "max"]), ("reshape", &[]), ("ravel", &[]), ("flatten", &[]), ("copy", &[]),
    ("transpose", &[]), ("swapaxes", &["axis1", "axis2"]), ("astype", &[]), ("tolist", &[]), ("item", &[]), ("fill", &[]), ("sort", &["axis", "kind"]), ("argsort", &["axis", "kind"]), ("nonzero", &[]), ("dot", &[]),
    ("squeeze", &["axis"]), ("diagonal", &["offset"]), ("trace", &[]), ("repeat", &["axis"]), ("conj", &[]), ("conjugate", &[]), ("is_integer", &[]), ("as_integer_ratio", &[]),
    // Generator and RandomState methods (rt_nprand.rs); the same names on Python's random.Random are fine too
    ("integers", &["low", "high", "size", "dtype", "endpoint"]), ("random", &["size"]), ("random_sample", &["size"]), ("sample", &["size"]), ("ranf", &["size"]), ("rand", &[]), ("randn", &[]),
    ("randint", &["low", "high", "size", "dtype"]), ("normal", &["loc", "scale", "size"]), ("uniform", &["low", "high", "size"]), ("standard_normal", &["size"]),
    ("exponential", &["scale", "size"]), ("standard_exponential", &["size"]), ("choice", &["a", "size", "replace", "p"]), ("shuffle", &["x"]), ("permutation", &["x"]), ("seed", &["seed"]),
    // the distributions (rt_nprdist.rs)
    ("standard_gamma", &["shape", "size"]), ("gamma", &["shape", "scale", "size"]), ("beta", &["a", "b", "size"]), ("chisquare", &["df", "size"]), ("noncentral_chisquare", &["df", "nonc", "size"]), ("f", &["dfnum", "dfden", "size"]), ("noncentral_f", &["dfnum", "dfden", "nonc", "size"]), ("standard_cauchy", &["size"]), ("standard_t", &["df", "size"]), ("vonmises", &["mu", "kappa", "size"]), ("pareto", &["a", "size"]), ("weibull", &["a", "size"]), ("power", &["a", "size"]), ("laplace", &["loc", "scale", "size"]), ("gumbel", &["loc", "scale", "size"]), ("logistic", &["loc", "scale", "size"]), ("lognormal", &["mean", "sigma", "size"]), ("rayleigh", &["scale", "size"]), ("wald", &["mean", "scale", "size"]), ("triangular", &["left", "mode", "right", "size"]), ("poisson", &["lam", "size"]), ("zipf", &["a", "size"]), ("geometric", &["p", "size"]), ("logseries", &["p", "size"]), ("negative_binomial", &["n", "p", "size"]), ("binomial", &["n", "p", "size"]), ("hypergeometric", &["ngood", "nbad", "nsample", "size"]), ("multinomial", &["n", "pvals", "size"]), ("dirichlet", &["alpha", "size"]),
    // names Python's own values have too: fine on those, and the native numpy has them or they are not numpy's
    ("count", &[]), ("index", &[]), ("hex", &[]), ("fromhex", &[]), ("numerator", &[]), ("denominator", &[]), ("bit_count", &[]),
];
// np.random's functions (the legacy global RandomState, and default_rng), with their keyword arguments;
// Generator and RandomState methods are in NP_ATTR_OK.
const NP_RAND_FUNCS: &[(&str, &[&str])] = &[
    ("default_rng", &["seed"]), ("RandomState", &["seed"]), ("seed", &["seed"]), ("rand", &[]), ("randn", &[]), ("randint", &["low", "high", "size", "dtype"]),
    ("random", &["size"]), ("random_sample", &["size"]), ("sample", &["size"]), ("ranf", &["size"]), ("uniform", &["low", "high", "size"]),
    ("normal", &["loc", "scale", "size"]), ("standard_normal", &["size"]), ("exponential", &["scale", "size"]), ("standard_exponential", &["size"]),
    ("choice", &["a", "size", "replace", "p"]), ("shuffle", &["x"]), ("permutation", &["x"]),
    ("standard_gamma", &["shape", "size"]), ("gamma", &["shape", "scale", "size"]), ("beta", &["a", "b", "size"]), ("chisquare", &["df", "size"]), ("noncentral_chisquare", &["df", "nonc", "size"]), ("f", &["dfnum", "dfden", "size"]), ("noncentral_f", &["dfnum", "dfden", "nonc", "size"]), ("standard_cauchy", &["size"]), ("standard_t", &["df", "size"]), ("vonmises", &["mu", "kappa", "size"]), ("pareto", &["a", "size"]), ("weibull", &["a", "size"]), ("power", &["a", "size"]), ("laplace", &["loc", "scale", "size"]), ("gumbel", &["loc", "scale", "size"]), ("logistic", &["loc", "scale", "size"]), ("lognormal", &["mean", "sigma", "size"]), ("rayleigh", &["scale", "size"]), ("wald", &["mean", "scale", "size"]), ("triangular", &["left", "mode", "right", "size"]), ("poisson", &["lam", "size"]), ("zipf", &["a", "size"]), ("geometric", &["p", "size"]), ("logseries", &["p", "size"]), ("negative_binomial", &["n", "p", "size"]), ("binomial", &["n", "p", "size"]), ("hypergeometric", &["ngood", "nbad", "nsample", "size"]), ("multinomial", &["n", "pvals", "size"]), ("dirichlet", &["alpha", "size"]),
];
const NP_LINALG_FUNCS: &[(&str, &[&str])] = &[("norm", &["x", "ord", "axis", "keepdims"]), ("det", &[]), ("slogdet", &[]), ("solve", &[]), ("inv", &[]), ("matrix_power", &[]), ("lstsq", &["a", "b", "rcond"]), ("LinAlgError", &[])];
pub(crate) fn np_linalg_kw(n: &str) -> Option<&'static [&'static str]> {
    NP_LINALG_FUNCS.iter().find(|(f, _)| *f == n).map(|(_, k)| *k)
}
pub(crate) fn np_rand_kw(n: &str) -> Option<&'static [&'static str]> {
    NP_RAND_FUNCS.iter().find(|(f, _)| *f == n).map(|(_, k)| *k)
}
pub(crate) fn np_func_kw(n: &str) -> Option<&'static [&'static str]> {
    NP_FUNCS.iter().find(|(f, _)| *f == n).map(|(_, k)| *k)
}
// Refusal (which sends the program to CPython's numpy) for an attribute or method of a numpy value that the
// native numpy lacks.
// Set while the program uses Brevi's safetensors: its safe_open has .metadata(), which numpy's dtypes have too
// (a dtype's .metadata is rare; on one it raises AttributeError).
pub static NP_ATTR_ST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// Brevi's own copies of libraries (jinja2, polars, ...): their `.partition()` or `.fields` is never numpy's.
pub static OWN_COPIES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
// The copies emitted ahead of LIB_END into the cached library: the large ones only (dynpy::compile picks them).
// Small copies (re-export shims such as mcp.types, contextlib) go with the program, so programs that import
// different small pieces of one library still share the one build of its large modules.
pub static LIB_COPIES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
thread_local! {
    static IN_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
// Attributes a native coroutine or async generator cannot give: the frame and code objects CPython runs it in,
// and the awaitable it waits on (Brevi runs coroutine bodies as compiled code, with no frame). Refused when the
// program is built, so a program reading one goes to CPython rather than stopping with AttributeError.
fn frame_attr_refused(n: &str) -> Option<String> {
    matches!(n, "cr_frame" | "cr_code" | "cr_await" | "cr_origin" | "ag_frame" | "ag_code" | "ag_await")
        .then(|| format!("the {n} attribute of a coroutine or async generator is not supported in dynamic builds"))
}
// Library attributes whose CPython value cannot work on the program's own values: types.coroutine marks a
// generator's code object as awaitable, and Brevi's generators have no code object for CPython to mark.
fn lib_attr_check(m: &str, n: &str, line: usize) -> ER<()> {
    if m == "types" && n == "coroutine" {
        return Err(err(line, "types.coroutine is not supported in dynamic builds"));
    }
    Ok(())
}
fn np_attr_check(n: &str, kws: &[&str], line: usize) -> ER<()> {
    if !np_on() || !NP_ATTR_ALL.contains(&n) || IN_COPY.with(|c| c.get()) {
        return Ok(());
    }
    if n == "metadata" && NP_ATTR_ST.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    if np_narrow() && matches!(n, "dot" | "trace") && !(n == "dot" && np_f32_simd_here()) {
        return Err(err(line, &format!("numpy: .{n} of float32 arrays is not in Brevi's native numpy")));
    }
    match NP_ATTR_OK.iter().find(|(a, _)| *a == n) {
        None => Err(err(line, &format!("numpy: .{n} is not in Brevi's native numpy"))),
        Some((_, ok)) => match kws.iter().find(|k| !ok.contains(k)) {
            Some(k) => Err(err(line, &format!("numpy: .{n}({k}=...) is not in Brevi's native numpy"))),
            None => Ok(()),
        },
    }
}

// Names a builtin module provides in dynamic builds (anything else is refused when the program is built).
pub fn module_has(m: &str, n: &str) -> bool {
    let names: &[&str] = match m {
        "__future__" => return true,
        "typing" | "typing_extensions" => return super::typing::module_has(m, n),
        "numpy" => return np_func_kw(n).is_some() && (!np_narrow() || NP_NARROW_FUNCS.contains(&n) || (NP_F32_SIMD.contains(&n) && np_f32_simd_here()) || (matches!(n, "dot" | "matmul") && np_f32_simd_here())),
        "numpy.random" => return np_rand_kw(n).is_some(),
        "numpy.linalg" => return np_linalg_kw(n).is_some() && (!np_narrow() || n == "norm"),
        "sys" => &["exit", "setrecursionlimit", "getrecursionlimit", "getsizeof", "intern", "set_int_max_str_digits", "get_int_max_str_digits", "stdin", "stdout", "stderr", "__stdin__", "__stdout__", "__stderr__", "argv", "maxsize", "platform", "byteorder", "version_info", "flags", "version", "modules", "path", "exception", "exc_info"],
        "math" => &["acos", "asin", "atan", "atan2", "ceil", "comb", "copysign", "cos", "cosh", "degrees", "dist", "exp", "fabs", "factorial", "floor", "fmod", "frexp", "fsum", "gcd", "hypot", "isclose", "isfinite", "isinf", "isnan", "isqrt", "lcm", "ldexp", "log", "log10", "log1p", "log2", "modf", "perm", "pow", "prod", "radians", "remainder", "sin", "sinh", "sqrt", "tan", "tanh", "trunc", "pi", "e", "tau", "inf", "nan"],
        "atexit" => &["register", "unregister", "_run_exitfuncs", "_clear", "_ncallbacks"],
        "heapq" => &["heapify", "heappop", "heappush", "heappushpop", "heapreplace", "merge", "nlargest", "nsmallest"],
        "bisect" => &["bisect", "bisect_left", "bisect_right", "insort", "insort_left", "insort_right"],
        "functools" => &["RLock", "WRAPPER_ASSIGNMENTS", "WRAPPER_UPDATES", "_CacheInfo", "_c3_mro", "_compose_mro", "_find_impl", "cache", "cached_property", "cmp_to_key", "lru_cache", "partial", "partialmethod", "reduce", "singledispatch", "singledispatchmethod", "total_ordering", "update_wrapper", "wraps"],
        "itertools" => &["accumulate", "batched", "chain", "combinations", "combinations_with_replacement", "compress", "count", "cycle", "dropwhile", "filterfalse", "groupby", "islice", "pairwise", "permutations", "product", "repeat", "starmap", "takewhile", "tee", "zip_longest"],
        "copy" => &["copy", "deepcopy"],
        "operator" => &["add", "and_", "attrgetter", "contains", "eq", "floordiv", "ge", "getitem", "gt", "index", "is_", "is_not", "itemgetter", "le", "length_hint", "lshift", "lt", "methodcaller", "mod", "mul", "ne", "neg", "not_", "or_", "pow", "rshift", "sub", "truediv", "truth", "xor", "__lt__", "__le__", "__gt__", "__ge__", "__eq__", "__ne__", "__and__", "__or__", "__xor__", "__sub__"],
        "string" => &["ascii_lowercase", "ascii_uppercase", "ascii_letters", "digits", "hexdigits", "octdigits", "punctuation", "whitespace", "printable"],
        "time" => &["time", "perf_counter", "monotonic", "process_time", "sleep", "time_ns", "perf_counter_ns", "monotonic_ns", "process_time_ns", "gmtime", "strftime"],
        "abc" => &["ABC", "ABCMeta", "abstractmethod", "_abc_register", "_check_methods"],
        "enum" => &["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag", "auto", "unique", "property", "ReprEnum"],
        "_brevi_term" => &["columns"],
        "hashlib" => &["new", "md5", "sha1", "sha224", "sha256", "sha384", "sha512"],
        "base64" => &["b64encode", "b64decode", "standard_b64encode", "standard_b64decode", "urlsafe_b64encode", "urlsafe_b64decode", "encodebytes", "decodebytes", "b16encode", "b16decode"],
        "binascii" => &["Error", "hexlify", "unhexlify", "b2a_hex", "a2b_hex"],
        "_brevi_async" => &["suspend", "iscoroutine", "isawaitable", "isasyncgen", "set_hooks", "track", "untrack", "tracked", "set_task_hooks", "set_gather_class", "gather", "make_handle", "run_ready", "task_step", "task_wakeup", "task_init", "schedule_callbacks", "fut_set_result", "fut_set_exception"],
        "_brevi_stdio" => &["claim", "release", "read", "write", "poll"],
        "_brevi_proc" => &["spawn", "read", "write", "close", "wait", "signal", "poll"],
        "_brevi_srv" => &["listen", "accept", "read", "write", "close", "poll", "random"],
        "_brevi_fs" => &["read", "write", "read_at", "size", "write_bytes", "listdir", "stat", "load", "save"],
        "_brevi_ort" => &["load", "providers", "build_info", "session", "run", "free"],
        "_brevi_st" => &["header", "header_len", "tensor", "dtype_name", "slice", "serialize"],
        "_brevi_sqlite" => &[
            "load", "open", "close", "errmsg", "errcode", "errstr", "prepare", "finalize", "reset", "clear", "step", "bind", "nparams", "param_name",
            "ncols", "col_name", "col_decl", "row", "col", "run", "dml", "fetch", "fetch_all", "db_int", "stmt_int", "busy_timeout", "interrupt", "limit", "complete", "hold", "runhold", "fetch_next",
            "cursor_next", "cursor_fetchone", "cursor_new", "conn_execute", "set_cursor_class", "set_adapted",
        ],
        "_brevi_duckdb" => &["load", "open", "connect", "disconnect", "close", "query", "many", "describe", "cols", "info", "fetch", "free", "interrupt", "term"],
        "_brevi_tok" => &["core", "ranks", "max_rank", "n_ranks", "find_special"],
        "_brevi_net" => &["open", "open_nb", "ready", "tls", "handshake", "settimeout", "setblocking", "send", "sendall", "fill", "head", "line", "take", "avail", "fileno", "close", "poll", "env", "host_ok", "zopen", "zfeed", "zstate", "zrest", "zclose"],
        "_brevi_hf" => &[
            "load", "load_built", "encode", "encode_batch", "decode", "decode_batch", "token_to_id", "id_to_token", "vocab", "vocab_size",
            "n_added", "set_truncation", "truncation", "set_padding", "padding", "add_tokens", "set_encode_special", "encode_special",
            "added_tokens", "lacks",
        ],
        "_brevi_col" => &["fsum", "cmp", "farith", "iarith", "take", "mask", "group1", "infer"],
        "_brevi_mt" => &["new", "seed", "next", "random", "bits", "below", "shuffle"],
        "collections" => &["defaultdict", "Counter", "OrderedDict", "deque", "namedtuple", "abc", "_count_elements"],
        "dataclasses" => &["asdict", "astuple", "dataclass", "field", "fields", "replace", "is_dataclass", "make_dataclass", "MISSING", "KW_ONLY", "InitVar", "Field", "FrozenInstanceError"],
        "re" => &["compile", "_compile", "escape", "findall", "finditer", "fullmatch", "match", "purge", "search", "split", "sub", "subn", "IGNORECASE", "I", "LOCALE", "L", "MULTILINE", "M", "DOTALL", "S", "VERBOSE", "X", "ASCII", "A", "NOFLAG", "UNICODE", "U", "DEBUG", "RegexFlag", "error", "PatternError", "Pattern", "Match", "Scanner"],
        "re._constants" => &["MAXGROUPS", "MAXREPEAT"],
        "re._compiler" => &["_generate_overlap_table", "MAXGROUPS", "MAXREPEAT"],
        "json" => &["dump", "dumps", "load", "loads", "JSONDecodeError", "JSONDecoder", "JSONEncoder"],
        "json.decoder" => &["scanstring", "JSONDecoder", "JSONDecodeError"],
        "decimal" => &["Decimal", "getcontext", "InvalidOperation", "DivisionByZero", "DecimalException", "ROUND_HALF_EVEN", "ROUND_HALF_UP", "ROUND_HALF_DOWN", "ROUND_UP", "ROUND_DOWN", "ROUND_CEILING", "ROUND_FLOOR", "ROUND_05UP"],
        "fractions" => &["Fraction"],
        "datetime" => &["date", "datetime", "time", "timedelta", "timezone", "UTC", "MINYEAR", "MAXYEAR"],
        "csv" => &["DictReader", "DictWriter", "reader", "writer", "QUOTE_MINIMAL", "QUOTE_ALL", "QUOTE_NONNUMERIC", "QUOTE_NONE", "Error"],
        "statistics" => &["fmean", "mean", "median", "median_high", "median_low", "mode", "multimode", "pstdev", "pvariance", "stdev", "variance", "StatisticsError"],
        "threading" => &["Barrier", "BoundedSemaphore", "BrokenBarrierError", "Condition", "Event", "Lock", "RLock", "Semaphore", "Thread", "active_count", "current_thread", "main_thread", "stack_size"],
        "html" => &["escape", "unescape"],
        "tzlocal" => &["get_localzone_name", "get_localzone"],
        "importlib.metadata" => &["version", "PackageNotFoundError"],
        "zoneinfo" => &["ZoneInfo", "ZoneInfoNotFoundError", "available_timezones", "TZPATH", "reset_tzpath"],
        "gc" => &["collect"],
        "weakref" => &["ref", "proxy"],
        "inspect" => &["Signature", "isabstract"],
        _ => return false,
    };
    names.contains(&n)
}

// gc has a native name (module_has); its other names keep running in CPython, as they did before the
// native one was added. weakref's do not: CPython's proxy, WeakValueDictionary, finalize, ... would hold the
// short-lived Python stand-in of a program value, not the value, so they are refused.
fn hybrid_bridged(m: &str) -> bool {
    m == "gc" && super::bridge_ok()
}
// Python builtins dynamic builds do not have: refused when the program is built.
fn is_missing_builtin(n: &str) -> bool {
    matches!(n, "exec" | "eval" | "compile" | "__import__" | "help" | "breakpoint" | "open" | "__build_class__" | "globals" | "locals")
}

fn is_builtin_name(n: &str) -> bool {
    matches!(
        n,
        "print" | "input" | "len" | "repr" | "ascii" | "abs" | "min" | "max" | "sum" | "sorted" | "reversed" | "enumerate" | "zip" | "map" | "filter" | "any" | "all" | "iter" | "next" | "isinstance" | "issubclass" | "hasattr" | "getattr" | "setattr" | "delattr" | "callable" | "ord" | "chr" | "hex" | "oct" | "bin" | "round" | "divmod" | "pow" | "hash" | "id" | "format" | "vars" | "dir" | "exit" | "quit" | "staticmethod" | "classmethod" | "property" | "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "slice" | "super" | "NotImplemented" | "Ellipsis" | "__name__" | "bytes" | "bytearray" | "memoryview" | "__bytes_lit" | "__brevi_lone_str" | "complex" | "__complex_lit" | "aiter" | "anext" | "__brevi_aenter" | "__brevi_aexit" | "__brevi_awith" | "__brevi_afor_await" | "__brevi_eg_split" | "__brevi_eg_reraise" | "__brevi_typevar" | "__brevi_tparams" | "__brevi_typealias" | "__brevi_generic" | "__brevi_pm_seq" | "__brevi_pm_ismap" | "__brevi_pm_map" | "__brevi_pm_cls"
    ) || is_exc_name(n)
}
fn is_exc_name(n: &str) -> bool {
    matches!(
        n,
        "BaseException" | "Exception" | "SystemExit" | "KeyboardInterrupt" | "GeneratorExit" | "ArithmeticError" | "ZeroDivisionError" | "OverflowError" | "FloatingPointError" | "LookupError" | "KeyError" | "IndexError" | "ValueError" | "UnicodeError" | "UnicodeDecodeError" | "UnicodeEncodeError" | "TypeError" | "AttributeError" | "NameError" | "UnboundLocalError" | "RuntimeError" | "RecursionError" | "NotImplementedError" | "SystemError" | "StopIteration" | "StopAsyncIteration" | "TimeoutError" | "ConnectionError" | "ConnectionRefusedError" | "ConnectionResetError" | "ConnectionAbortedError" | "BrokenPipeError" | "BlockingIOError" | "ChildProcessError" | "InterruptedError" | "ProcessLookupError" | "BaseExceptionGroup" | "ExceptionGroup" | "AssertionError" | "SyntaxError" | "IndentationError" | "ImportError" | "ModuleNotFoundError" | "OSError" | "IOError" | "EnvironmentError" | "FileNotFoundError" | "PermissionError" | "IsADirectoryError" | "NotADirectoryError" | "FileExistsError" | "EOFError" | "MemoryError" | "BufferError" | "Warning" | "UserWarning" | "DeprecationWarning" | "SyntaxWarning" | "RuntimeWarning" | "FutureWarning" | "PendingDeprecationWarning" | "ImportWarning" | "UnicodeWarning" | "BytesWarning" | "ResourceWarning" | "EncodingWarning" | "ReferenceError"
    )
}

fn lit(s: &str) -> String {
    format!("{:?}", s)
}

// ---------- scope analysis ----------

#[derive(Default, Clone)]
struct Scope {
    locals: BTreeSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
}

fn target_names(t: &Ex, out: &mut BTreeSet<String>) {
    match t {
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Tuple(v) | Ex::List(v) => v.iter().for_each(|x| target_names(x, out)),
        Ex::Star(x) => target_names(x, out),
        _ => {}
    }
}
fn walrus_names(e: &Ex, out: &mut BTreeSet<String>) {
    visit_ex(e, &mut |x| {
        if let Ex::Walrus(n, _) = x {
            out.insert(n.clone());
        }
    });
}
// Visit every expression node, not entering lambdas.
fn visit_ex(e: &Ex, f: &mut dyn FnMut(&Ex)) {
    f(e);
    match e {
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| visit_ex(x, f)),
        Ex::Dict(v) => v.iter().for_each(|(k, x)| {
            if let Some(k) = k {
                visit_ex(k, f);
            }
            visit_ex(x, f)
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => visit_ex(x, f),
        Ex::Yield(x) => {
            if let Some(x) = x {
                visit_ex(x, f)
            }
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            visit_ex(a, f);
            visit_ex(b, f)
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                visit_ex(x, f);
            }
        }
        Ex::Call(c, args) => {
            visit_ex(c, f);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => visit_ex(x, f),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            visit_ex(a, f);
            rest.iter().for_each(|(_, x)| visit_ex(x, f))
        }
        Ex::IfExp(a, b, c) => {
            visit_ex(a, f);
            visit_ex(b, f);
            visit_ex(c, f)
        }
        Ex::Comp(_, a, b, gens) => {
            visit_ex(a, f);
            if let Some(b) = b {
                visit_ex(b, f);
            }
            for g in gens {
                visit_ex(&g.target, f);
                visit_ex(&g.iter, f);
                g.ifs.iter().for_each(|x| visit_ex(x, f));
            }
        }
        Ex::FStr(parts) => {
            for p in parts {
                if let FPart::Expr(x, _, spec, _) = p {
                    visit_ex(x, f);
                    for sp in spec {
                        if let FPart::Expr(y, ..) = sp {
                            visit_ex(y, f);
                        }
                    }
                }
            }
        }
        Ex::Lambda(p, _) => {
            for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
                if let Some(d) = d {
                    visit_ex(d, f);
                }
            }
        }
        _ => {}
    }
}
// Visit every expression of the statements, nested defs, classes and lambda bodies included.
// Whether the module names __annotations__ anywhere (an attribute or a name).
fn mentions_annotations(body: &[Stmt]) -> bool {
    let mut found = false;
    visit_any(body, &mut |e| {
        // read by name too: getattr(f, "__annotations__"), or through functools.WRAPPER_ASSIGNMENTS
        // vars(C), C.__dict__ and dir(C) list a class's __annotations__ too
        if matches!(e, Ex::Attr(_, n) | Ex::Name(n) if n == "__annotations__" || n == "WRAPPER_ASSIGNMENTS")
            || matches!(e, Ex::Name(n) if n == "vars" || n == "dir")
            || matches!(e, Ex::Attr(_, n) if n == "__dict__")
            || matches!(e, Ex::Str(t) if t == "__annotations__")
        {
            found = true;
        }
    });
    found
}

pub fn visit_any(body: &[Stmt], f: &mut dyn FnMut(&Ex)) {
    fn lam(e: &Ex, f: &mut dyn FnMut(&Ex)) {
        visit_ex(e, &mut |x| {
            f(x);
            if let Ex::Lambda(_, b) = x {
                lam(b, f);
            }
        });
    }
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    for e in ex {
        lam(e, f);
    }
    fn nested<'a>(body: &'a [Stmt], out: &mut Vec<&'a [Stmt]>) {
        for s in body {
            match &s.st {
                St::Def(d) => out.push(&d.body),
                St::Class(c) => out.push(&c.body),
                St::If(arms, els) => {
                    arms.iter().for_each(|(_, b)| nested(b, out));
                    if let Some(b) = els {
                        nested(b, out);
                    }
                }
                St::For(_, _, b, e) | St::While(_, b, e) => {
                    nested(b, out);
                    if let Some(b) = e {
                        nested(b, out);
                    }
                }
                St::Try(b, hs, e, fi) => {
                    nested(b, out);
                    hs.iter().for_each(|h| nested(&h.body, out));
                    for b in [e, fi].into_iter().flatten() {
                        nested(b, out);
                    }
                }
                St::With(_, b) => nested(b, out),
                _ => {}
            }
        }
    }
    let mut inner = vec![];
    nested(body, &mut inner);
    for b in inner {
        visit_any(b, f);
    }
}
// Expressions directly in a statement list (not entering nested defs/classes, except decorators,
// defaults and bases, which run in this scope).
fn stmt_exprs<'a>(body: &'a [Stmt], out: &mut Vec<&'a Ex>) {
    for s in body {
        match &s.st {
            St::Expr(e) | St::Return(Some(e)) => out.push(e),
            St::Assign(ts, v) => {
                ts.iter().for_each(|t| out.push(t));
                out.push(v);
            }
            St::AugAssign(t, _, v) => {
                out.push(t);
                out.push(v);
            }
            St::AnnAssign(t, v, _, _) => {
                out.push(t);
                if let Some(v) = v {
                    out.push(v);
                }
            }
            St::Raise(a, b) => {
                if let Some(a) = a {
                    out.push(a);
                }
                if let Some(b) = b {
                    out.push(b);
                }
            }
            St::Del(v) => v.iter().for_each(|x| out.push(x)),
            St::Assert(a, b) => {
                out.push(a);
                if let Some(b) = b {
                    out.push(b);
                }
            }
            St::If(arms, els) => {
                for (c, b) in arms {
                    out.push(c);
                    stmt_exprs(b, out);
                }
                if let Some(b) = els {
                    stmt_exprs(b, out);
                }
            }
            St::While(c, b, e) => {
                out.push(c);
                stmt_exprs(b, out);
                if let Some(e) = e {
                    stmt_exprs(e, out);
                }
            }
            St::For(t, i, b, e) => {
                out.push(t);
                out.push(i);
                stmt_exprs(b, out);
                if let Some(e) = e {
                    stmt_exprs(e, out);
                }
            }
            St::Try(b, hs, e, f) => {
                stmt_exprs(b, out);
                for h in hs {
                    if let Some(t) = &h.types {
                        out.push(t);
                    }
                    stmt_exprs(&h.body, out);
                }
                if let Some(e) = e {
                    stmt_exprs(e, out);
                }
                if let Some(f) = f {
                    stmt_exprs(f, out);
                }
            }
            St::With(items, b) => {
                for (e, t) in items {
                    out.push(e);
                    if let Some(t) = t {
                        out.push(t);
                    }
                }
                stmt_exprs(b, out);
            }
            St::Def(d) => {
                d.decorators.iter().for_each(|x| out.push(x));
                for (_, x) in d.params.pos.iter().chain(d.params.kwonly.iter()) {
                    if let Some(x) = x {
                        out.push(x);
                    }
                }
            }
            St::Class(c) => {
                c.decorators.iter().for_each(|x| out.push(x));
                c.bases.iter().for_each(|x| out.push(x));
                c.kwargs.iter().for_each(|(_, x)| out.push(x));
            }
            _ => {}
        }
    }
}

// Visit the expression nodes that run in the current class-body scope: no lambda bodies, and only a
// comprehension's outermost iterable (the rest runs in the comprehension's own frame).
fn visit_class_ex(e: &Ex, f: &mut dyn FnMut(&Ex)) {
    f(e);
    match e {
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| visit_class_ex(x, f)),
        Ex::Dict(v) => v.iter().for_each(|(k, x)| {
            if let Some(k) = k {
                visit_class_ex(k, f);
            }
            visit_class_ex(x, f)
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => visit_class_ex(x, f),
        Ex::Yield(x) => {
            if let Some(x) = x {
                visit_class_ex(x, f)
            }
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            visit_class_ex(a, f);
            visit_class_ex(b, f)
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                visit_class_ex(x, f);
            }
        }
        Ex::Call(c, args) => {
            visit_class_ex(c, f);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => visit_class_ex(x, f),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            visit_class_ex(a, f);
            rest.iter().for_each(|(_, x)| visit_class_ex(x, f))
        }
        Ex::IfExp(a, b, c) => {
            visit_class_ex(a, f);
            visit_class_ex(b, f);
            visit_class_ex(c, f)
        }
        Ex::Comp(_, _, _, gens) => {
            if let Some(g) = gens.first() {
                visit_class_ex(&g.iter, f);
            }
        }
        Ex::FStr(parts) => {
            for p in parts {
                if let FPart::Expr(x, _, spec, _) = p {
                    visit_class_ex(x, f);
                    for sp in spec {
                        if let FPart::Expr(y, ..) = sp {
                            visit_class_ex(y, f);
                        }
                    }
                }
            }
        }
        Ex::Lambda(p, _) => {
            for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
                if let Some(d) = d {
                    visit_class_ex(d, f);
                }
            }
        }
        _ => {}
    }
}

// Whether the class body itself calls locals() (not a nested def, class, lambda body or the inner
// part of a comprehension): then its namespace must be a dict the call returns and may change.
fn class_calls_locals(body: &[Stmt]) -> bool {
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    let mut found = false;
    for e in ex {
        visit_class_ex(e, &mut |x| {
            if matches!(x, Ex::Call(f, a) if a.is_empty() && matches!(f.as_ref(), Ex::Name(n) if n == "locals")) {
                found = true;
            }
        });
    }
    found
}
// `def n` in this body (not in a nested function) whose own body calls n through the enclosing cell
fn self_recursive_def(body: &[Stmt], n: &str) -> bool {
    fn find<'a>(body: &'a [Stmt], n: &str) -> Option<&'a FuncDef> {
        for s in body {
            let inner: Vec<&[Stmt]> = match &s.st {
                St::Def(d) if d.name == n => return Some(d),
                St::If(arms, els) => arms.iter().map(|(_, b)| b.as_slice()).chain(els.as_deref()).collect(),
                St::For(_, _, b, e) | St::While(_, b, e) => std::iter::once(b.as_slice()).chain(e.as_deref()).collect(),
                _ => vec![],
            };
            if let Some(d) = inner.into_iter().find_map(|b| find(b, n)) {
                return Some(d);
            }
        }
        None
    }
    let Some(d) = find(body, n) else { return false };
    if scope_of(&d.body, &lambda_params(&d.params)).locals.contains(n) {
        return false;
    }
    let mut uses = false;
    visit_any(&d.body, &mut |x| uses |= matches!(x, Ex::Name(m) if m == n));
    uses
}
// A def nested in a function that later calls can reach as a plain Rust function (see Em::nested_direct): the
// only binding of its name in that function, no decorators, positional parameters only, no yield or async,
// and no nested function rebinding the name with nonlocal.
fn direct_nested_def<'a>(body: &'a [Stmt], n: &str) -> Option<&'a FuncDef> {
    fn same_scope<'a>(body: &'a [Stmt], f: &mut dyn FnMut(&'a Stmt)) {
        for s in body {
            f(s);
            match &s.st {
                St::If(arms, els) => {
                    arms.iter().for_each(|(_, b)| same_scope(b, f));
                    if let Some(b) = els {
                        same_scope(b, f);
                    }
                }
                St::For(_, _, b, e) | St::While(_, b, e) => {
                    same_scope(b, f);
                    if let Some(e) = e {
                        same_scope(e, f);
                    }
                }
                St::Try(b, hs, e, fi) => {
                    same_scope(b, f);
                    hs.iter().for_each(|h| same_scope(&h.body, f));
                    if let Some(e) = e {
                        same_scope(e, f);
                    }
                    if let Some(fi) = fi {
                        same_scope(fi, f);
                    }
                }
                St::With(_, b) => same_scope(b, f),
                _ => {}
            }
        }
    }
    let mut defs = vec![];
    let mut other = false;
    same_scope(body, &mut |s| match &s.st {
        St::Def(d) if d.name == n => defs.push(d),
        // any other statement that binds n here (one at a time, so its sub-bodies are not counted twice)
        St::If(..) | St::While(..) | St::Try(..) | St::Def(_) => {}
        St::For(t, ..) => {
            let mut b = BTreeSet::new();
            target_names(t, &mut b);
            other |= b.contains(n);
        }
        St::With(items, _) => {
            for (_, t) in items {
                if let Some(t) = t {
                    let mut b = BTreeSet::new();
                    target_names(t, &mut b);
                    other |= b.contains(n);
                }
            }
        }
        _ => other |= scope_of(std::slice::from_ref(s), &[]).locals.contains(n),
    });
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    let mut w = BTreeSet::new();
    for e in ex {
        walrus_names(e, &mut w);
    }
    let mut handler_bind = false;
    same_scope(body, &mut |s| {
        if let St::Try(_, hs, _, _) = &s.st {
            handler_bind |= hs.iter().any(|h| h.name.as_deref() == Some(n));
        }
    });
    if defs.len() != 1 || other || w.contains(n) || handler_bind {
        return None;
    }
    let d = defs[0];
    if !d.decorators.is_empty() || d.is_async || has_yield(&d.body) || d.params.star.is_some() || d.params.kwstar.is_some() || !d.params.kwonly.is_empty() {
        return None;
    }
    if nonlocal_in(body, n) {
        return None;
    }
    Some(d)
}
// nonlocal n (or global n) in any function nested in body
fn nonlocal_in(body: &[Stmt], n: &str) -> bool {
    let mut found = false;
    fn walk(body: &[Stmt], n: &str, found: &mut bool) {
        for s in body {
            match &s.st {
                St::Nonlocal(ns) | St::Global(ns) if ns.iter().any(|x| x == n) => *found = true,
                St::Def(d) => walk(&d.body, n, found),
                St::Class(c) => walk(&c.body, n, found),
                St::If(arms, els) => {
                    arms.iter().for_each(|(_, b)| walk(b, n, found));
                    if let Some(b) = els {
                        walk(b, n, found);
                    }
                }
                St::For(_, _, b, e) | St::While(_, b, e) => {
                    walk(b, n, found);
                    if let Some(e) = e {
                        walk(e, n, found);
                    }
                }
                St::Try(b, hs, e, fi) => {
                    walk(b, n, found);
                    hs.iter().for_each(|h| walk(&h.body, n, found));
                    if let Some(e) = e {
                        walk(e, n, found);
                    }
                    if let Some(fi) = fi {
                        walk(fi, n, found);
                    }
                }
                St::With(_, b) => walk(b, n, found),
                _ => {}
            }
        }
    }
    walk(body, n, &mut found);
    found
}
fn scope_of(body: &[Stmt], params: &[String]) -> Scope {
    let mut sc = Scope::default();
    for p in params {
        sc.locals.insert(p.clone());
    }
    fn walk(body: &[Stmt], sc: &mut Scope) {
        for s in body {
            match &s.st {
                St::Assign(ts, _) => ts.iter().for_each(|t| target_names(t, &mut sc.locals)),
                St::AugAssign(t, _, _) => target_names(t, &mut sc.locals),
                // a name in parentheses with no value (`(x): int`) binds nothing
                St::AnnAssign(t, v, _, simple) if *simple || v.is_some() => target_names(t, &mut sc.locals),
                St::For(t, _, b, e) => {
                    target_names(t, &mut sc.locals);
                    walk(b, sc);
                    if let Some(e) = e {
                        walk(e, sc);
                    }
                }
                St::While(_, b, e) => {
                    walk(b, sc);
                    if let Some(e) = e {
                        walk(e, sc);
                    }
                }
                St::If(arms, els) => {
                    for (_, b) in arms {
                        walk(b, sc);
                    }
                    if let Some(b) = els {
                        walk(b, sc);
                    }
                }
                St::Try(b, hs, e, f) => {
                    walk(b, sc);
                    for h in hs {
                        if let Some(n) = &h.name {
                            sc.locals.insert(n.clone());
                        }
                        walk(&h.body, sc);
                    }
                    if let Some(e) = e {
                        walk(e, sc);
                    }
                    if let Some(f) = f {
                        walk(f, sc);
                    }
                }
                St::With(items, b) => {
                    for (_, t) in items {
                        if let Some(t) = t {
                            target_names(t, &mut sc.locals);
                        }
                    }
                    walk(b, sc);
                }
                St::Def(d) => {
                    sc.locals.insert(d.name.clone());
                }
                St::Class(c) => {
                    sc.locals.insert(c.name.clone());
                }
                St::Import(v) => {
                    for (m, a) in v {
                        sc.locals.insert(a.clone().unwrap_or_else(|| m.split('.').next().unwrap().to_string()));
                    }
                }
                St::ImportFrom(_, v) => {
                    for (n, a) in v {
                        sc.locals.insert(a.clone().unwrap_or_else(|| n.clone()));
                    }
                }
                St::Del(v) => v.iter().for_each(|t| target_names(t, &mut sc.locals)),
                St::Global(ns) => ns.iter().for_each(|n| {
                    sc.globals.insert(n.clone());
                }),
                St::Nonlocal(ns) => ns.iter().for_each(|n| {
                    sc.nonlocals.insert(n.clone());
                }),
                _ => {}
            }
        }
    }
    walk(body, &mut sc);
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    for e in ex {
        walrus_names(e, &mut sc.locals);
    }
    for g in sc.globals.iter().chain(sc.nonlocals.iter()) {
        sc.locals.remove(g);
    }
    sc
}
// Names read in a function body (and its nested functions, minus their own locals): the free names.
fn names_read_ex(e: &Ex, out: &mut BTreeSet<String>) {
    visit_ex(e, &mut |x| match x {
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Walrus(n, _) => {
            out.insert(n.clone());
        }
        Ex::Lambda(p, body) => {
            let mut inner = BTreeSet::new();
            names_read_ex(body, &mut inner);
            let params: HashSet<String> = lambda_params(p).into_iter().collect();
            for n in inner {
                if !params.contains(&n) {
                    out.insert(n);
                }
            }
        }
        _ => {}
    });
}
// Free names of an expression relative to the scope around it: names it reads that are not bound by
// lambdas or comprehensions inside it. A nested comprehension's outermost iterable runs in the
// surrounding scope, so its names count too.
fn expr_free(e: &Ex, out: &mut BTreeSet<String>) {
    match e {
        Ex::Name(n) => {
            out.insert(n.clone());
        }
        Ex::Walrus(n, v) => {
            out.insert(n.clone());
            expr_free(v, out);
        }
        Ex::Lambda(p, body) => {
            for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
                if let Some(d) = d {
                    expr_free(d, out);
                }
            }
            let mut inner = BTreeSet::new();
            expr_free(body, &mut inner);
            let ps: HashSet<String> = lambda_params(p).into_iter().collect();
            for n in inner {
                if !ps.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Ex::Comp(_, elt, val, gens) => {
            let mut inner = BTreeSet::new();
            expr_free(elt, &mut inner);
            if let Some(v) = val {
                expr_free(v, &mut inner);
            }
            for g in gens {
                expr_free(&g.iter, &mut inner);
                g.ifs.iter().for_each(|x| expr_free(x, &mut inner));
            }
            let mut bound = BTreeSet::new();
            for g in gens {
                target_names(&g.target, &mut bound);
            }
            for n in inner {
                if !bound.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| expr_free(x, out)),
        Ex::Dict(v) => v.iter().for_each(|(k, x)| {
            if let Some(k) = k {
                expr_free(k, out);
            }
            expr_free(x, out)
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => expr_free(x, out),
        Ex::Yield(x) => {
            if let Some(x) = x {
                expr_free(x, out)
            }
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            expr_free(a, out);
            expr_free(b, out)
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                expr_free(x, out);
            }
        }
        Ex::Call(c, args) => {
            expr_free(c, out);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => expr_free(x, out),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            expr_free(a, out);
            rest.iter().for_each(|(_, x)| expr_free(x, out))
        }
        Ex::IfExp(a, b, c) => {
            expr_free(a, out);
            expr_free(b, out);
            expr_free(c, out)
        }
        Ex::FStr(parts) => {
            for p in parts {
                if let FPart::Expr(x, _, spec, _) = p {
                    expr_free(x, out);
                    for sp in spec {
                        if let FPart::Expr(y, ..) = sp {
                            expr_free(y, out);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
// The names a generator expression's lazy part reads from the scope around it: element, conditions and
// the iterables after the first. (The outermost iterable is evaluated where the genexp is written, so its
// names are not free.)
fn genexp_free(gens: &[CompFor], elt: &Ex, val: Option<&Ex>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    expr_free(elt, &mut out);
    if let Some(v) = val {
        expr_free(v, &mut out);
    }
    for (k, g) in gens.iter().enumerate() {
        if k > 0 {
            expr_free(&g.iter, &mut out);
        }
        g.ifs.iter().for_each(|x| expr_free(x, &mut out));
    }
    let mut bound = BTreeSet::new();
    for g in gens {
        target_names(&g.target, &mut bound);
    }
    out.retain(|n| !bound.contains(n));
    out
}
// Names read by scopes nested in an expression (lambdas and comprehensions), relative to the expression's
// scope. Used to decide which comprehension targets a nested scope reads, so those targets are cells.
fn nested_scope_free(e: &Ex, out: &mut BTreeSet<String>) {
    match e {
        Ex::Lambda(p, body) => {
            let mut inner = BTreeSet::new();
            expr_free(body, &mut inner);
            let ps: HashSet<String> = lambda_params(p).into_iter().collect();
            for n in inner {
                if !ps.contains(&n) {
                    out.insert(n);
                }
            }
            for (_, d) in p.pos.iter().chain(p.kwonly.iter()) {
                if let Some(d) = d {
                    nested_scope_free(d, out);
                }
            }
        }
        Ex::Comp(_, elt, val, gens) => {
            let mut inner = BTreeSet::new();
            expr_free(elt, &mut inner);
            if let Some(v) = val {
                expr_free(v, &mut inner);
            }
            for g in gens {
                expr_free(&g.iter, &mut inner);
                g.ifs.iter().for_each(|x| expr_free(x, &mut inner));
            }
            let mut bound = BTreeSet::new();
            for g in gens {
                target_names(&g.target, &mut bound);
            }
            for n in inner {
                if !bound.contains(&n) {
                    out.insert(n);
                }
            }
        }
        Ex::List(v) | Ex::Tuple(v) | Ex::Set(v) => v.iter().for_each(|x| nested_scope_free(x, out)),
        Ex::Dict(v) => v.iter().for_each(|(k, x)| {
            if let Some(k) = k {
                nested_scope_free(k, out);
            }
            nested_scope_free(x, out)
        }),
        Ex::Star(x) | Ex::Attr(x, _) | Ex::Unary(_, x) | Ex::Walrus(_, x) | Ex::YieldFrom(x) | Ex::Await(x) => nested_scope_free(x, out),
        Ex::Yield(x) => {
            if let Some(x) = x {
                nested_scope_free(x, out)
            }
        }
        Ex::Index(a, b) | Ex::Bin(_, a, b) | Ex::And(a, b) | Ex::Or(a, b) => {
            nested_scope_free(a, out);
            nested_scope_free(b, out)
        }
        Ex::Slice(a, b, c) => {
            for x in [a, b, c].into_iter().flatten() {
                nested_scope_free(x, out);
            }
        }
        Ex::Call(c, args) => {
            nested_scope_free(c, out);
            for a in args {
                match a {
                    Arg::Pos(x) | Arg::Star(x) | Arg::Kw(_, x) | Arg::KwStar(x) => nested_scope_free(x, out),
                }
            }
        }
        Ex::Cmp(a, rest) => {
            nested_scope_free(a, out);
            rest.iter().for_each(|(_, x)| nested_scope_free(x, out))
        }
        Ex::IfExp(a, b, c) => {
            nested_scope_free(a, out);
            nested_scope_free(b, out);
            nested_scope_free(c, out)
        }
        Ex::FStr(parts) => {
            for p in parts {
                if let FPart::Expr(x, _, spec, _) = p {
                    nested_scope_free(x, out);
                    for sp in spec {
                        if let FPart::Expr(y, ..) = sp {
                            nested_scope_free(y, out);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
fn lambda_params(p: &Params) -> Vec<String> {
    let mut v: Vec<String> = p.pos.iter().map(|x| x.0.clone()).collect();
    v.extend(p.kwonly.iter().map(|x| x.0.clone()));
    if let Some(s) = &p.star {
        v.push(s.clone());
    }
    if let Some(s) = &p.kwstar {
        v.push(s.clone());
    }
    v
}
fn free_of_def(params: &[String], body: &[Stmt]) -> BTreeSet<String> {
    let sc = scope_of(body, params);
    let mut used = BTreeSet::new();
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    for e in ex {
        names_read_ex(e, &mut used);
    }
    for n in nested_free(body) {
        used.insert(n);
    }
    // a name declared nonlocal is free here even when this body never reads or sets it
    used.extend(sc.nonlocals.iter().cloned());
    used.into_iter().filter(|n| !sc.locals.contains(n) || sc.nonlocals.contains(n)).filter(|n| !sc.globals.contains(n)).collect()
}
// Whether a method body needs the class cell (a zero-argument super() call). Brevi captures the class
// cell for every method so its Rust can reach the class, but CPython puts it in __closure__ only when the
// body uses it; __closure__ is built from the smaller list below.
fn body_has_zero_super(body: &[Stmt]) -> bool {
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    ex.iter().any(|e| {
        let mut found = false;
        visit_ex(e, &mut |x| {
            if let Ex::Call(f, args) = x {
                if matches!(f.as_ref(), Ex::Name(n) if n == "super") && args.is_empty() {
                    found = true;
                }
            }
        });
        found
    })
}

// free names of the defs and classes directly nested in body
fn nested_free(body: &[Stmt]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn walk(body: &[Stmt], out: &mut BTreeSet<String>) {
        for s in body {
            match &s.st {
                St::Def(d) => {
                    for n in free_of_def(&lambda_params(&d.params), &d.body) {
                        out.insert(n);
                    }
                }
                St::Class(c) => {
                    // class body: its names are its own; methods' free names reach out
                    out.insert(c.name.clone());
                    let mut used = BTreeSet::new();
                    let mut ex = vec![];
                    stmt_exprs(&c.body, &mut ex);
                    for e in ex {
                        names_read_ex(e, &mut used);
                    }
                    for n in nested_free(&c.body) {
                        used.insert(n);
                    }
                    let own = scope_of(&c.body, &[]).locals;
                    for n in used {
                        if !own.contains(&n) {
                            out.insert(n);
                        }
                    }
                    for n in nested_free(&c.body) {
                        out.insert(n);
                    }
                }
                St::If(arms, els) => {
                    for (_, b) in arms {
                        walk(b, out);
                    }
                    if let Some(b) = els {
                        walk(b, out);
                    }
                }
                St::While(_, b, e) | St::For(_, _, b, e) => {
                    walk(b, out);
                    if let Some(e) = e {
                        walk(e, out);
                    }
                }
                St::Try(b, hs, e, f) => {
                    walk(b, out);
                    for h in hs {
                        walk(&h.body, out);
                    }
                    if let Some(e) = e {
                        walk(e, out);
                    }
                    if let Some(f) = f {
                        walk(f, out);
                    }
                }
                St::With(_, b) => walk(b, out),
                _ => {}
            }
        }
    }
    walk(body, &mut out);
    // lambdas directly in this body
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    for e in ex {
        visit_ex(e, &mut |x| {
            if let Ex::Lambda(p, b) = x {
                let mut inner = BTreeSet::new();
                names_read_ex(b, &mut inner);
                let ps: HashSet<String> = lambda_params(p).into_iter().collect();
                for n in inner {
                    if !ps.contains(&n) {
                        out.insert(n);
                    }
                }
            }
            // A generator expression's lazy part runs in its own scope: locals it reads must be cells.
            if let Ex::Comp(kind, elt, val, gens) = x {
                if *kind == 'g' {
                    for n in genexp_free(gens, elt, val.as_deref()) {
                        out.insert(n);
                    }
                }
            }
        });
    }
    out
}
// Generators switch stacks with a few lines of assembly (rt_gen.rs), or with a Windows fiber, for these targets.
const GEN_OK: bool = cfg!(any(target_arch = "x86_64", target_arch = "aarch64"));

// An async def anywhere in this code (nested ones too).
pub fn uses_async(body: &[Stmt]) -> bool {
    body.iter().any(|s| match &s.st {
        St::Def(d) => d.is_async || uses_async(&d.body),
        St::Class(c) => uses_async(&c.body),
        St::If(arms, els) => arms.iter().any(|(_, b)| uses_async(b)) || els.as_deref().is_some_and(uses_async),
        St::For(_, _, b, e) | St::While(_, b, e) => uses_async(b) || e.as_deref().is_some_and(uses_async),
        St::Try(b, hs, e, f) => uses_async(b) || hs.iter().any(|h| uses_async(&h.body)) || e.as_deref().is_some_and(uses_async) || f.as_deref().is_some_and(uses_async),
        St::With(_, b) => uses_async(b),
        _ => false,
    })
}

// Async code next to a library CPython runs builds natively unless a coroutine could cross between the two:
// a coroutine function, method or lambda handed to the library (or a class built on one of its classes with
// async methods, or an async def it decorates), or something the library made being awaited (CPython's
// awaitables are not Brevi's). What the library made is followed by name, file-wide: the names its imports
// bind, then names and attributes assigned from those, and functions returning them. Returns the line and
// what crosses, for the first such use.
pub fn async_crosses_bridge(mods: &[&[Stmt]], bridged: &dyn Fn(&str) -> bool) -> Option<(usize, String)> {
    fn all<'a>(body: &'a [Stmt], out: &mut Vec<&'a Stmt>) {
        for s in body {
            out.push(s);
            match &s.st {
                St::Def(d) => all(&d.body, out),
                St::Class(c) => all(&c.body, out),
                St::If(arms, els) => {
                    arms.iter().for_each(|(_, b)| all(b, out));
                    if let Some(b) = els {
                        all(b, out);
                    }
                }
                St::For(_, _, b, e) | St::While(_, b, e) => {
                    all(b, out);
                    if let Some(e) = e {
                        all(e, out);
                    }
                }
                St::Try(b, hs, e, f) => {
                    all(b, out);
                    hs.iter().for_each(|h| all(&h.body, out));
                    for x in [e, f].into_iter().flatten() {
                        all(x, out);
                    }
                }
                St::With(_, b) => all(b, out),
                _ => {}
            }
        }
    }
    // the expressions of one statement, not of the blocks inside it
    fn own(s: &Stmt) -> Vec<&Ex> {
        let mut v = vec![];
        match &s.st {
            St::If(arms, _) => arms.iter().for_each(|(c, _)| v.push(c)),
            St::While(c, _, _) => v.push(c),
            St::For(t, i, _, _) => {
                v.push(t);
                v.push(i)
            }
            St::With(items, _) => items.iter().for_each(|(e, t)| {
                v.push(e);
                if let Some(t) = t {
                    v.push(t)
                }
            }),
            St::Try(_, hs, _, _) => hs.iter().filter_map(|h| h.types.as_ref()).for_each(|t| v.push(t)),
            St::Def(_) | St::Class(_) => stmt_exprs(std::slice::from_ref(s), &mut v),
            _ => stmt_exprs(std::slice::from_ref(s), &mut v),
        }
        v
    }
    let mut stmts = vec![];
    for b in mods {
        all(b, &mut stmts);
    }
    let mut names: HashSet<String> = HashSet::new();
    let mut attrs: HashSet<String> = HashSet::new();
    let mut coros: HashSet<String> = HashSet::new();
    for s in &stmts {
        match &s.st {
            St::Import(ns) => {
                for (m, a) in ns {
                    if bridged(m) {
                        names.insert(a.clone().unwrap_or_else(|| m.split('.').next().unwrap_or(m).to_string()));
                    }
                }
            }
            St::ImportFrom(m, ns) if bridged(m) => {
                for (n, a) in ns {
                    names.insert(a.clone().unwrap_or_else(|| n.clone()));
                }
            }
            St::Def(d) if d.is_async => {
                coros.insert(d.name.clone());
            }
            _ => {}
        }
    }
    if names.is_empty() {
        return None;
    }
    let refers = |e: &Ex, names: &HashSet<String>, attrs: &HashSet<String>| {
        let mut hit = false;
        visit_ex(e, &mut |x| match x {
            Ex::Name(n) if names.contains(n) => hit = true,
            Ex::Attr(_, n) if attrs.contains(n) => hit = true,
            _ => {}
        });
        hit
    };
    fn bind(t: &Ex, names: &mut HashSet<String>, attrs: &mut HashSet<String>) -> bool {
        match t {
            Ex::Name(n) => names.insert(n.clone()),
            Ex::Attr(_, n) => attrs.insert(n.clone()),
            Ex::Tuple(v) | Ex::List(v) => v.iter().fold(false, |a, x| bind(x, names, attrs) | a),
            Ex::Star(x) => bind(x, names, attrs),
            _ => false,
        }
    }
    loop {
        let mut grew = false;
        for s in &stmts {
            match &s.st {
                St::Assign(ts, v) if refers(v, &names, &attrs) => ts.iter().for_each(|t| grew |= bind(t, &mut names, &mut attrs)),
                St::AnnAssign(t, Some(v), _, _) if refers(v, &names, &attrs) => grew |= bind(t, &mut names, &mut attrs),
                St::For(t, i, _, _) if refers(i, &names, &attrs) => grew |= bind(t, &mut names, &mut attrs),
                St::With(items, _) => {
                    for (e, t) in items {
                        if let Some(t) = t {
                            if refers(e, &names, &attrs) {
                                grew |= bind(t, &mut names, &mut attrs);
                            }
                        }
                    }
                }
                // (an async def returning what it made gives a coroutine of Brevi's, which may be awaited)
                St::Def(d) if !d.is_async && !names.contains(&d.name) => {
                    let mut body = vec![];
                    all(&d.body, &mut body);
                    if body.iter().any(|x| matches!(&x.st, St::Return(Some(e)) if refers(e, &names, &attrs))) {
                        names.insert(d.name.clone());
                        grew = true;
                    }
                }
                _ => {}
            }
        }
        if !grew {
            break;
        }
    }
    // what the library made, at the head of an expression: `x`, `x.y`, `x.f()`, `x[0]`, `lib.f()`
    fn head(e: &Ex, names: &HashSet<String>, attrs: &HashSet<String>) -> bool {
        match e {
            Ex::Name(n) => names.contains(n),
            Ex::Attr(b, n) => attrs.contains(n) || head(b, names, attrs),
            Ex::Call(f, _) | Ex::Index(f, _) => head(f, names, attrs),
            _ => false,
        }
    }
    let asyncio_call = |f: &Ex| {
        let mut b = f;
        while let Ex::Attr(x, _) = b {
            b = x;
        }
        matches!(b, Ex::Name(n) if n == "asyncio")
    };
    let is_coro = |e: &Ex| match e {
        Ex::Lambda(..) => true,
        Ex::Name(n) | Ex::Attr(_, n) => coros.contains(n),
        Ex::Call(f, _) => matches!(&**f, Ex::Name(n) | Ex::Attr(_, n) if coros.contains(n)),
        _ => false,
    };
    for s in &stmts {
        match &s.st {
            St::Def(d) if d.is_async && d.decorators.iter().any(|x| refers(x, &names, &attrs)) => return Some((s.line, format!("async def {} decorated by it", d.name))),
            St::Class(c) if c.bases.iter().any(|x| refers(x, &names, &attrs)) && uses_async(&c.body) => return Some((s.line, format!("class {} with async methods on its class", c.name))),
            _ => {}
        }
        for e in own(s) {
            let mut found: Option<String> = None;
            visit_ex(e, &mut |x| {
                if found.is_some() {
                    return;
                }
                match x {
                    Ex::Await(a) if head(a, &names, &attrs) => found = Some("awaiting what it made".into()),
                    Ex::Call(f, args)
                        if asyncio_call(f) && args.iter().any(|a| matches!(a, Arg::Pos(v) | Arg::Kw(_, v) | Arg::Star(v) | Arg::KwStar(v) if head(v, &names, &attrs))) =>
                    {
                        found = Some("handing what it made to asyncio".into())
                    }
                    Ex::Call(f, args) if refers(f, &names, &attrs) => {
                        if args.iter().any(|a| matches!(a, Arg::Pos(v) | Arg::Kw(_, v) | Arg::Star(v) | Arg::KwStar(v) if is_coro(v))) {
                            found = Some("a coroutine function or lambda handed to it".into());
                        }
                    }
                    _ => {}
                }
            });
            if let Some(w) = found {
                return Some((s.line, w));
            }
        }
    }
    None
}

fn has_yield(body: &[Stmt]) -> bool {
    let mut ex = vec![];
    stmt_exprs(body, &mut ex);
    let mut found = false;
    for e in ex {
        visit_ex(e, &mut |x| {
            if matches!(x, Ex::Yield(_) | Ex::YieldFrom(_)) {
                found = true;
            }
        });
    }
    found
}

// a break, continue or return that can leave this block (nested defs and classes excluded)
fn has_jump(body: &[Stmt]) -> bool {
    body.iter().any(|s| match &s.st {
        St::Break | St::Continue | St::Return(_) => true,
        St::If(arms, els) => arms.iter().any(|(_, b)| has_jump(b)) || els.as_deref().is_some_and(has_jump),
        St::For(_, _, b, e) | St::While(_, b, e) => has_jump(b) || e.as_deref().is_some_and(has_jump),
        St::Try(b, hs, e, f) => has_jump(b) || hs.iter().any(|h| has_jump(&h.body)) || e.as_deref().is_some_and(has_jump) || f.as_deref().is_some_and(has_jump),
        St::With(_, b) => has_jump(b),
        _ => false,
    })
}

// ---------- emitter ----------

#[derive(Clone, PartialEq)]
enum Var {
    Local(String),
    Cell(String),
    Global(usize),
    // a name of a class body run in namespace-dict mode (it calls locals()): stored in and read
    // from the class namespace dict held in the Rust local named by the string
    NS(String),
}

#[derive(Clone)]
enum Frame {
    // try/except body: errors go to the labeled block
    Exc(String),
    // try/finally or with body: errors, returns, breaks go through the labeled block
    Fin(String),
    Loop(String),
}

struct Func {
    vars: HashMap<String, Var>,
    frames: Vec<Frame>,
    // class whose method this is, and the first parameter's name (for super())
    class_expr: Option<String>,
    self_name: Option<String>,
    // the qualname of this frame (without the module): "" for a module body, "C" for a class
    // body, "outer.<locals>.f" for a def, so nested defs can build CPython's qualnames
    qname: String,
    exc_vars: Vec<String>,
    is_module: bool,
    is_class: bool,
    // class-body stores turn auto() into its value here (last-values vec, enum kind), when the class
    // being defined is an Enum (None elsewhere)
    enum_cnv: Option<(String, String)>,
    // an async def's body: await may be used
    is_async: bool,
    // an async def with yield: an async generator
    is_agen: bool,
    // prefix of the names of the cells this function owns ("k{id}_"); empty for module and class bodies
    cells: String,
    // a function's names in CPython's locals() order: parameters and locals by first appearance, then cell and
    // free variables by name
    order: Vec<String>,
    // a def with yield: a generator
    is_gen: bool,
    // nested defs held in plain locals of this frame that calls here reach directly (see Em::nested_direct)
    direct_defs: HashMap<String, (String, Vec<String>, usize)>,
    // a class body run in namespace-dict mode: the Rust local holding its namespace dict
    ns: Option<String>,
}

pub struct Module {
    pub name: String,
    pub body: Vec<Stmt>,
    // A file of the program (its __main__ or a module it imports): the file's absolute path, as CPython's
    // __file__ and module repr give it. None for Brevi's own copies of library modules.
    pub path: Option<String>,
}

// Python libraries the program imports that Brevi lacks, run by CPython inside the program (pyglue.rs), and
// classes built on their classes, run by CPython whole. Empty (`on` false) when CPython may not run (the
// sandbox without allow native) or is missing.
#[derive(Default)]
pub struct Bridge {
    pub on: bool,
    // top-level names of bridged modules (`requests`, `pydantic`)
    pub modules: HashSet<String>,
    // (module, line of `class`) -> the class's source, at its own line numbers
    pub classes: HashMap<(String, usize), String>,
    // module -> its import lines, for those classes
    pub imports: HashMap<String, String>,
    // module -> its file
    pub files: HashMap<String, String>,
    // module -> (its absolute path, its source), for the warnings the runtime issues at a line of it
    pub srcs: HashMap<String, (String, String)>,
}
impl Bridge {
    pub fn has(&self, m: &str) -> bool {
        self.on && self.modules.contains(m.split('.').next().unwrap_or(m))
    }
}

// The value-set key of a constant set display. CPython merges equal frozenset constants across
// a module (_PyCode_ConstantKey + merge_consts_recursive), so displays with the same values all
// share the first one's table. Types stay distinct, as in CPython: 1, 1.0 and True do not merge.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstKey {
    Int(bool, Vec<u32>),
    Float(u64),
    Str(String),
    NoneV,
    Bool(bool),
    Ellipsis,
    Tuple(Vec<ConstKey>),
}

// An int literal's digits as a little-endian base-1e9 number, so 16, 0x10 and 0b10000 compare equal.
fn int_key(t: &str) -> (bool, Vec<u32>) {
    let mut s = t.replace('_', "");
    let neg = s.starts_with('-');
    if neg || s.starts_with('+') {
        s = s[1..].to_string();
    }
    let base = if let Some(r) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        s = r.to_string();
        16
    } else if let Some(r) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        s = r.to_string();
        8
    } else if let Some(r) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        s = r.to_string();
        2
    } else {
        10
    };
    let mut mag: Vec<u32> = vec![0];
    for c in s.chars() {
        let mut carry = c.to_digit(16).unwrap_or(0) as u64;
        for limb in mag.iter_mut() {
            let v = *limb as u64 * base as u64 + carry;
            *limb = (v % 1_000_000_000) as u32;
            carry = v / 1_000_000_000;
        }
        if carry != 0 {
            mag.push(carry as u32);
        }
    }
    while mag.len() > 1 && mag.last() == Some(&0) {
        mag.pop();
    }
    let zero = mag.len() == 1 && mag[0] == 0;
    (neg && !zero, mag)
}

fn const_key(e: &Ex) -> ConstKey {
    match e {
        Ex::Int(t) => {
            let (neg, mag) = int_key(t);
            ConstKey::Int(neg, mag)
        }
        Ex::Float(f) => ConstKey::Float(f.to_bits()),
        Ex::Str(s) => ConstKey::Str(s.clone()),
        Ex::NoneV => ConstKey::NoneV,
        Ex::True => ConstKey::Bool(true),
        Ex::False => ConstKey::Bool(false),
        Ex::Ellipsis => ConstKey::Ellipsis,
        Ex::Unary("-", x) => match x.as_ref() {
            Ex::Int(t) => {
                let (neg, mag) = int_key(t);
                let zero = mag.len() == 1 && mag[0] == 0;
                ConstKey::Int(!neg && !zero, mag)
            }
            Ex::Float(f) => ConstKey::Float((-f).to_bits()),
            _ => unreachable!(),
        },
        Ex::Tuple(xs) => ConstKey::Tuple(xs.iter().map(const_key).collect()),
        _ => unreachable!(),
    }
}

pub struct Em {
    // some module mentions __del__: statements run waiting finalizers (block)
    has_del: bool,
    // some module mentions __closure__: defs record their cells for it (else reading it is refused at run time,
    // so making a closure costs nothing extra in the programs that never look)
    has_closure: bool,
    // cells with a CellGuard (see self_recursive_def)
    guards: HashSet<String>,
    // nested defs callable as plain Rust functions, by the cell holding them: (rust fn, captured cells, nparams).
    // A call passes the cells along, so it skips the function value, bind() and the argument Vec.
    nested_direct: HashMap<String, (String, Vec<String>, usize)>,
    // cells only their own function assigns (no nonlocal for them anywhere), by name, with the depth of that
    // function on fstack: functions nested in it may read them in place (cgs, see borrow_cells in mod.rs)
    safe_cells: HashMap<String, usize>,
    out: String,
    items: String,
    n: usize,
    globals: HashMap<(String, String), usize>,
    nglobals: usize,
    consts: Vec<String>,
    const_idx: HashMap<String, usize>,
    // str constants with a lone surrogate: hex code points -> slot in consts (whose text there is unused)
    lone_idx: HashMap<String, usize>,
    fstack: Vec<Func>,
    module: String,
    user_modules: Vec<String>,
    // the program's own files (__main__ and the modules it imports): name -> (absolute path, docstring,
    // __package__), the module attributes CPython's import sets on them
    mod_info: HashMap<String, (String, Option<String>, Option<String>)>,
    // module-level defs that can be called directly: (module, name) -> (rust fn, nparams, default slots, defaults count)
    direct: HashMap<(String, String), (String, usize, Vec<usize>)>,
    // module-level names bound only once by import of a builtin module
    mod_alias: HashMap<(String, String), String>,
    // global names `import types` binds (module, name): `types.coroutine` through them is refused
    types_alias: HashSet<(String, String)>,
    // the subset bound only by that import (once, never rebound): `@types.coroutine` through one is known statically
    types_once: HashSet<(String, String)>,
    // the def being built is a generator function under `@types.coroutine`: its generators are iterable coroutines
    next_iter_coro: bool,
    // names assigned anywhere at module level, per module
    mod_names: HashMap<String, BTreeSet<String>>,
    class_ctx: Option<String>,
    // the module being built has `from __future__ import annotations` (annotations stay strings, unevaluated)
    lazy: bool,
    // the program reads __annotations__ somewhere, so class bodies keep theirs (evaluated as CPython does);
    // otherwise annotations are dropped unevaluated as before, since an annotation the runtime cannot
    // evaluate (`int | None`, a forward name) must not stop a program that never looks at it
    keep_ann: bool,
    // CPython evaluates the annotations of this module's module and class bodies and defs (no `from __future__
    // import annotations`, not Brevi's own copy of a CPython module, which CPython evaluates without error, and
    // not a .bv file in Brevi's own spellings, whose annotations are Brevi's types): each is evaluated as its
    // statement or def runs, so `x: undefined` raises NameError as in CPython
    eval_ann: bool,
    // PEP 695 type parameters of the def whose own annotations are being evaluated: TypeVars there
    ann_tvars: Vec<String>,
    // the classes of the program seen so far that subclass type
    metaclasses: BTreeSet<String>,
    // the def being built is an async def (read by closure)
    next_async: bool,
    direct_ok: HashSet<(String, String)>,
    bridge: Bridge,
    // what the program has CPython run, for the path report
    bridged_used: BTreeSet<String>,
    // the program calls the re module (see the buffer check at the end of emit)
    uses_re: bool,
    // (module, name) bound by `from re import name`: calls of it say where they are, as `re.name(...)` does
    re_names: HashSet<(String, String)>,
    // the program uses the warnings module: every statement says where it is, so a warning names its line
    track_warn: bool,
    // module -> the lines whose logical line runs over more than one line (None: the module did not lex)
    multi: HashMap<String, Option<HashSet<usize>>>,
    // the WMod statics written so far
    wmods: HashSet<String>,
    // while a lambda whose body is on one line is emitted: that line, known even in a statement over several
    lam_line: Option<usize>,
    // (module, name) bound by `import warnings`
    warn_names: HashSet<(String, String)>,
    // module + value set -> the first display's items in source order (CPython's constant merging)
    set_lits: HashMap<(String, BTreeSet<ConstKey>), Vec<Ex>>,
}

// The annotation a def passes to functools.singledispatch's register when the def is decorated with a bare
// `@register` (no explicit class). Brevi never evaluates annotations, so the def carries a private descriptor
// instead: ("type", lambda) for a class, ("none",) for None, ("union", (arms...), repr) for a union, and
// ("invalid", repr) / ("unsupported", repr) for the rest. register evaluates only the descriptor it is given.
fn sd_is_typing(e: &Ex) -> bool {
    matches!(e, Ex::Attr(o, _) if matches!(o.as_ref(), Ex::Name(m) if m == "typing" || m == "typing_extensions"))
}
fn sd_typing_name(e: &Ex) -> Option<&str> {
    match e {
        Ex::Attr(_, n) if sd_is_typing(e) => Some(n),
        _ => None,
    }
}
fn sd_args(i: &Ex) -> Vec<&Ex> {
    match i {
        Ex::Tuple(v) => v.iter().collect(),
        x => vec![x],
    }
}
fn sd_has_typing(e: &Ex) -> bool {
    match e {
        Ex::Attr(..) if sd_is_typing(e) => true,
        Ex::Index(o, i) => sd_is_typing(o) || sd_has_typing(o) || sd_has_typing(i),
        Ex::Bin(_, a, b) => sd_has_typing(a) || sd_has_typing(b),
        _ => false,
    }
}
fn sd_repr(e: &Ex) -> String {
    match e {
        Ex::Name(n) => n.clone(),
        Ex::Attr(o, n) => format!("{}.{}", sd_repr(o), n),
        Ex::Index(o, i) => format!("{}[{}]", sd_repr(o), sd_repr_args(i)),
        // types.UnionType with a typing form inside prints as CPython's typing.Union[...] repr.
        Ex::Bin("|", a, b) if sd_has_typing(a) || sd_has_typing(b) => format!("typing.Union[{}, {}]", sd_repr(a), sd_repr(b)),
        Ex::Bin(op, a, b) => format!("{} {op} {}", sd_repr(a), sd_repr(b)),
        Ex::NoneV => "None".into(),
        Ex::Str(s) => format!("{s:?}"),
        Ex::Int(t) => t.clone(),
        Ex::Float(f) => f.to_string(),
        Ex::True => "True".into(),
        Ex::False => "False".into(),
        Ex::Ellipsis => "...".into(),
        Ex::Tuple(v) => v.iter().map(sd_repr).collect::<Vec<_>>().join(", "),
        _ => format!("{e:?}"),
    }
}
fn sd_repr_args(i: &Ex) -> String {
    match i {
        Ex::Tuple(v) => v.iter().map(sd_repr).collect::<Vec<_>>().join(", "),
        x => sd_repr(x),
    }
}
// A name or attribute that Python would evaluate to a class. Typing forms and collections.abc ABCs are not
// classes Brevi's dynamic runtime can dispatch on, so they stay out (as unsupported annotations).
fn sd_class_like(e: &Ex) -> bool {
    match e {
        Ex::Name(n) => !super::typing::TYPING_NAMES.contains(&n.as_str()),
        Ex::Attr(o, _) => !matches!(o.as_ref(), Ex::Name(m) if m == "typing" || m == "typing_extensions" || m == "collections" || m == "abc"),
        _ => false,
    }
}
fn is_dec_name(e: &Ex, n: &str) -> bool {
    match e {
        Ex::Attr(_, x) if x == n => true,
        Ex::Name(x) if x == n => true,
        _ => false,
    }
}

impl Em {
    fn id(&mut self) -> usize {
        self.n += 1;
        self.n
    }
    // The descriptor value (as a Rust expression) for a def's first annotation.
    fn sd_ann_desc(&mut self, e: &Ex, line: usize) -> ER<String> {
        Ok(match e {
            Ex::NoneV => "tup(vec![s(\"none\")])".into(),
            Ex::Bin("|", a, b) => {
                let la = self.sd_ann_desc(a, line)?;
                let lb = self.sd_ann_desc(b, line)?;
                format!("tup(vec![s(\"union\"), tup(vec![{la}, {lb}]), s({})])", lit(&sd_repr(e)))
            }
            Ex::Index(head, i) => {
                let hn = sd_typing_name(head);
                if matches!(hn, Some("Union" | "Optional")) {
                    let mut arms: Vec<String> = vec![];
                    for x in sd_args(i) {
                        arms.push(self.sd_ann_desc(x, line)?);
                    }
                    if hn == Some("Optional") {
                        arms.push("tup(vec![s(\"none\")])".into());
                    }
                    format!("tup(vec![s(\"union\"), tup(vec![{}]), s({})])", arms.join(", "), lit(&sd_repr(e)))
                } else {
                    format!("tup(vec![s(\"invalid\"), s({})])", lit(&sd_repr(e)))
                }
            }
            _ if sd_class_like(e) => {
                let lam = Ex::Lambda(Box::new(Params::default()), Box::new(e.clone()));
                let lv = self.ex(&lam, line)?;
                format!("tup(vec![s(\"type\"), {lv}])")
            }
            // a string annotation, as typing.get_type_hints reads it: evaluated in the def's module globals.
            // Taken when it names a class through names that are the module's own (or builtins) here too.
            Ex::Str(text) => {
                let parsed = super::parse::parse(text.trim()).ok().and_then(|mut v| match v.pop() {
                    Some(st) if v.is_empty() => match st.st {
                        St::Expr(x) => Some(x),
                        _ => None,
                    },
                    _ => None,
                });
                fn base(e: &Ex) -> Option<&str> {
                    match e {
                        Ex::Name(n) => Some(n),
                        Ex::Attr(o, _) => base(o),
                        _ => None,
                    }
                }
                match parsed {
                    Some(x) if sd_class_like(&x) && base(&x).is_some_and(|n| matches!(self.lookup(n), None | Some(Var::Global(_)))) => {
                        self.sd_ann_desc(&x, line)?
                    }
                    _ => format!("tup(vec![s(\"unsupported\"), s({})])", lit(&sd_repr(e))),
                }
            }
            _ => format!("tup(vec![s(\"unsupported\"), s({})])", lit(&sd_repr(e))),
        })
    }
    fn gslot(&mut self, module: &str, name: &str) -> usize {
        if let Some(&i) = self.globals.get(&(module.to_string(), name.to_string())) {
            return i;
        }
        let i = self.nglobals;
        self.nglobals += 1;
        self.globals.insert((module.to_string(), name.to_string()), i);
        i
    }
    fn konst(&mut self, s: &str) -> String {
        let i = match self.const_idx.get(s) {
            Some(&i) => i,
            None => {
                let i = self.consts.len();
                self.consts.push(s.to_string());
                self.const_idx.insert(s.to_string(), i);
                i
            }
        };
        format!("kc({i})")
    }
    // A str literal with a lone surrogate (the parser's __brevi_lone_str call): one constant per distinct value,
    // as CPython keeps one object for equal constants.
    fn konst_lone(&mut self, hex: &str) -> String {
        let i = match self.lone_idx.get(hex) {
            Some(&i) => i,
            None => {
                let i = self.consts.len();
                self.consts.push(String::new());
                self.lone_idx.insert(hex.to_string(), i);
                i
            }
        };
        format!("kc({i})")
    }
    fn f(&mut self) -> &mut Func {
        self.fstack.last_mut().unwrap()
    }
    // CPython's __qualname__ for a def, lambda, class or genexpr named `name` defined in the current scope:
    // `outer.<locals>.f` inside a function, `Outer.f` inside a class, plain `name` at module level.
    fn qualname_of(&self, name: &str) -> String {
        let mut parent = self.fstack.last().unwrap();
        // a PEP 695 annotation scope (the parser's hidden __brevi_gp_ def) is not in CPython's qualnames
        if parent.qname.rsplit('.').next().is_some_and(|l| l.starts_with("__brevi_gp_")) && self.fstack.len() >= 2 {
            parent = &self.fstack[self.fstack.len() - 2];
        }
        if parent.is_module {
            name.to_string()
        } else if parent.is_class {
            format!("{}.{}", parent.qname, name)
        } else {
            format!("{}.<locals>.{}", parent.qname, name)
        }
    }
    // `expr?` or a jump to the innermost try block
    fn q(&self, code: &str) -> String {
        for fr in self.fstack.last().unwrap().frames.iter().rev() {
            match fr {
                Frame::Exc(l) => return format!("tq!({code}, {l})"),
                Frame::Fin(l) => return format!("fq!({code}, {l})"),
                Frame::Loop(_) => {}
            }
        }
        format!("{code}?")
    }
    // raise an E value
    fn raise_code(&self, e: &str) -> String {
        for fr in self.fstack.last().unwrap().frames.iter().rev() {
            match fr {
                Frame::Exc(l) => return format!("break {l} Err({e})"),
                Frame::Fin(l) => return format!("break {l} Flow::Exc({e})"),
                Frame::Loop(_) => {}
            }
        }
        format!("return Err({e})")
    }
    fn lookup(&self, name: &str) -> Option<Var> {
        // innermost function first; outer functions' variables are reached as cells
        let top = self.fstack.len() - 1;
        for (k, f) in self.fstack.iter().enumerate().rev() {
            if f.is_class && k != top {
                continue;
            }
            if let Some(v) = f.vars.get(name) {
                return Some(v.clone());
            }
            if f.is_module {
                break;
            }
        }
        None
    }
    fn load(&mut self, name: &str, line: usize) -> ER<String> {
        // A class body in namespace-dict mode reads every name through its dict first
        // (CPython's LOAD_FROM_DICT_OR_DEREF, and LOAD_NAME for the class's own names).
        let ns = self.f().ns.clone();
        if let Some(r) = ns {
            let id = self.id();
            let get = self.q(&format!("class_ns_get(&{r}, {})", lit(name)));
            let fallback = self.load_named(name, line)?;
            return Ok(format!("{{ let __g{id} = {get}; if let Some(__g{id}) = __g{id} {{ __g{id} }} else {{ {fallback} }} }}"));
        }
        self.load_named(name, line)
    }
    fn load_named(&mut self, name: &str, line: usize) -> ER<String> {
        match self.lookup(name) {
            // a class body's own name not bound yet: LOAD_NAME goes on to the globals and builtins
            Some(Var::Local(r)) if self.fstack.last().unwrap().is_class => {
                let fb = match self.fstack[0].vars.get(name).cloned() {
                    Some(Var::Global(i)) => self.q(&format!("gl({i}, {})", lit(name))),
                    _ => self.load_missing(name, line)?,
                };
                Ok(format!("(if matches!({r}, V::Unbound) {{ {fb} }} else {{ {r}.clone() }})"))
            }
            Some(Var::Local(r)) => Ok(self.q(&format!("ld(&{r}, {})", lit(name)))),
            Some(Var::Cell(r)) => {
                let own = !self.f().cells.is_empty() && r.starts_with(&self.f().cells);
                let fr = self.fstack.last().unwrap();
                let inner = self.safe_cells.get(&r).is_some_and(|&d| self.fstack.len() > d) && !fr.is_gen && !fr.is_async;
                let f = if own { "cget_own" } else if inner { "cgs" } else { "cget" };
                Ok(self.q(&format!("{f}(&{r}, {})", lit(name))))
            }
            Some(Var::Global(i)) => {
                if is_builtin_name(name) || name == "__name__" {
                    Ok(self.q(&format!("glb({i}, {})", lit(name))))
                } else {
                    Ok(self.q(&format!("gl({i}, {})", lit(name))))
                }
            }
            // LOAD_NAME after a miss in the class namespace sees only globals and builtins,
            // never an enclosing function's cells.
            Some(Var::NS(_)) => match self.fstack[0].vars.get(name).cloned() {
                Some(Var::Global(i)) => Ok(self.q(&format!("gl({i}, {})", lit(name)))),
                _ => self.load_missing(name, line),
            },
            None => self.load_missing(name, line),
        }
    }
    fn load_missing(&mut self, name: &str, line: usize) -> ER<String> {
        if name == "__name__" {
            return Ok(self.konst(&self.module.clone()).replace("kc", "kc"));
        }
        // a program file's own __doc__ and __package__ (its __file__ is written into the text, modules.rs)
        if name != "__file__" && name != "__dict__" {
            if let Some(d) = self.mod_dunder(&self.module.clone(), name) {
                return d.map_err(|()| err(line, &format!("{name} is not supported in dynamic builds")));
            }
        }
        if is_builtin_name(name) {
            if is_exc_name(name) {
                let n = match name {
                    "IOError" | "EnvironmentError" => "OSError",
                    x => x,
                };
                return Ok(format!("V::Class(exc_class({}))", lit(n)));
            }
            if name == "super" {
                return Err(err(line, "super used as a value is not supported"));
            }
            return Ok(format!("builtin_value({}).unwrap()", lit(name)));
        }
        // Brevi's own open() (_brevi_open.py) when no CPython library is in the program
        if name == "open" && !self.bridge.on && self.user_modules.iter().any(|m| m == "_brevi_open") {
            let init = self.q("init__brevi_open()");
            let v = self.user_mod_load("_brevi_open", "open", line)?;
            return Ok(format!("{{ {init}; {v} }}"));
        }
        // Not under --sandbox --allow-py: the caller granted CPython for named modules, not open.
        if name == "open" && self.bridge.on && crate::caps::py_only().is_none() {
            self.bridged_used.insert("open".into());
            return Ok(self.q("pyd_builtin(\"open\")"));
        }
        if name == "globals" {
            return Err(err(line, "globals() is supported in dynamic builds only as globals()[name], globals().get(name) and name in globals()"));
        }
        if is_missing_builtin(name) {
            return Err(err(line, &format!("{name}() is not supported in dynamic builds")));
        }
        Ok(self.q(&format!("name_err({})", lit(name))))
    }
    fn store(&mut self, name: &str, val: &str) -> String {
        let cnv = match self.fstack.last() {
            Some(fr) if fr.is_class => fr.enum_cnv.clone(),
            _ => None,
        };
        match self.lookup(name) {
            Some(Var::Local(r)) => {
                if let Some((av, ctx)) = cnv {
                    format!("{r} = {};", self.q(&format!("enum_auto_store(&mut {av}, &mut {ctx}, {}, {val})", lit(name))))
                } else {
                    format!("{r} = {val};")
                }
            }
            Some(Var::Cell(r)) => format!("cset(&{r}, {val});"),
            Some(Var::Global(i)) => format!("gs({i}, {val});"),
            Some(Var::NS(r)) => {
                if let Some((av, ctx)) = cnv {
                    let v = self.q(&format!("enum_auto_store(&mut {av}, &mut {ctx}, {}, {val})", lit(name)));
                    let set = self.q(&format!("setitem(&{r}, &s({}), __v)", lit(name)));
                    format!("{{ let __v = {v}; {set}; }}")
                } else {
                    let set = self.q(&format!("setitem(&{r}, &s({}), {val})", lit(name)));
                    format!("{set};")
                }
            }
            None => {
                let m = self.module.clone();
                let i = self.gslot(&m, name);
                format!("gs({i}, {val});")
            }
        }
    }

    // ---- expressions ----
    fn ex(&mut self, e: &Ex, line: usize) -> ER<String> {
        Ok(match e {
            Ex::Name(n) if self.ann_tvars.contains(n) => format!("pep695_typevar({})", lit(n)),
            Ex::Name(n) => self.load(n, line)?,
            Ex::Int(t) => {
                let (body, base) = if let Some(r) = t.strip_prefix("0x") {
                    (r, 16)
                } else if let Some(r) = t.strip_prefix("0o") {
                    (r, 8)
                } else if let Some(r) = t.strip_prefix("0b") {
                    (r, 2)
                } else {
                    (t.as_str(), 10)
                };
                match i64::from_str_radix(body, base) {
                    Ok(v) => format!("V::Int({v}i64)"),
                    Err(_) => format!("parse_int({}, {base}).unwrap()", lit(body)),
                }
            }
            Ex::Float(f) => {
                if f.is_infinite() {
                    "V::Float(f64::INFINITY)".into()
                } else {
                    format!("V::Float({:?}f64)", f)
                }
            }
            Ex::Str(s) => self.konst(s),
            Ex::Call(f, a) if matches!((f.as_ref(), &a[..]), (Ex::Name(n), [Arg::Pos(Ex::Str(_))]) if n == "__brevi_lone_str") => {
                let [Arg::Pos(Ex::Str(hex))] = &a[..] else { unreachable!() };
                self.konst_lone(hex)
            }
            Ex::NoneV => "V::None".into(),
            Ex::True => "V::Bool(true)".into(),
            Ex::False => "V::Bool(false)".into(),
            Ex::Ellipsis => "V::Ellipsis".into(),
            Ex::FStr(parts) => {
                let body = self.fparts(parts, line)?;
                format!("txt_v({{ let mut __s = Txt::new(); {body} __s }})")
            }
            Ex::List(v) => format!("list({})", self.seq(v, line)?),
            Ex::Tuple(v) => {
                let sq = self.seq(v, line)?;
                // a tuple of fixed size goes straight into its one allocation
                match sq.strip_prefix("vec![").and_then(|r| r.strip_suffix(']')) {
                    Some(items) if !v.is_empty() => format!("tupa([{items}])"),
                    _ => format!("tup({sq})"),
                }
            }
            Ex::Set(v) => {
                // CPython builds a literal of 3 or more constants as a frozenset constant copied into a new
                // set, which can order the items differently from adding them one by one; and equal constant
                // sets anywhere in the module share the first one's frozenset (constant merging), so a display
                // whose values repeat an earlier one uses that earlier display's source order.
                fn konst(e: &Ex) -> bool {
                    match e {
                        Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False | Ex::Ellipsis => true,
                        Ex::Unary("-", x) => matches!(x.as_ref(), Ex::Int(_) | Ex::Float(_)),
                        Ex::Tuple(xs) => xs.iter().all(konst),
                        _ => false,
                    }
                }
                let folded = v.len() >= 3 && v.iter().all(konst);
                let items = if folded {
                    let key: BTreeSet<ConstKey> = v.iter().map(const_key).collect();
                    let m = self.module.clone();
                    let have = self.set_lits.get(&(m.clone(), key.clone())).cloned();
                    match have {
                        Some(x) => x,
                        None => {
                            self.set_lits.insert((m, key), v.clone());
                            v.clone()
                        }
                    }
                } else {
                    v.clone()
                };
                let sq = self.seq(&items, line)?;
                let one = self.q(&format!("set_of_vec({sq})"));
                if folded {
                    let fs = self.q(&format!("fset_rebuild({one})"));
                    self.q(&format!("bi_set(Args::p(vec![{fs}]))"))
                } else {
                    one
                }
            }
            Ex::Dict(items) => {
                let mut b = String::from("{ let mut __d = Dict::new(); ");
                for (k, v) in items {
                    match k {
                        Some(k) => {
                            let kk = self.ex(k, line)?;
                            let vv = self.ex(v, line)?;
                            let set = self.q(&format!("__d.set(__k, {vv})"));
                            write!(b, "{{ let __k = {kk}; {set}; }} ").unwrap();
                        }
                        None => {
                            let vv = self.ex(v, line)?;
                            let items = self.q(&format!("dict_items_of(&{vv})"));
                            let set = self.q("__d.set(__k, __v)");
                            write!(b, "for (__k, __v) in {items} {{ {set}; }} ").unwrap();
                        }
                    }
                }
                b.push_str("V::Dict(Rc::new(RefCell::new(__d))) }");
                b
            }
            Ex::Star(_) => return Err(err(line, "starred expression here is not supported")),
            // Brevi's own module copies (pydantic, the MCP server, string, ...) read class namespaces for their
            // own bookkeeping: they get the namespace even where the program's C.__dict__ refuses (rt_tdict.rs)
            Ex::Attr(o, n) if n == "__dict__" && IN_COPY.with(|k| k.get()) => {
                let ov = self.ex(o, line)?;
                self.q(&format!("own_dict_of(&{ov})"))
            }
            Ex::Attr(o, n) => {
                // typing.ByteString stays refused: Brevi's typing does not give the collections.abc class, whose isinstance()
                // and subclassing warn as CPython 3.13's metaclass does
                if n == "ByteString" && self.bridge.srcs.contains_key(&self.module) {
                    let m = self.static_module(o).filter(|m| m == "typing");
                    if let Some(m) = m {
                        return Err(err(line, &format!("{m}.ByteString is not supported in dynamic builds")));
                    }
                }
                if let Some(m) = self.static_module(o) {
                    lib_attr_check(&m, n, line)?;
                    if m == "collections" && n == "abc" && self.user_modules.iter().any(|u| u == "collections.abc") {
                        let init = self.q(&format!("init_{}()", rust_ident("collections.abc")));
                        return Ok(self.q(&format!("{{ {init}; Ok::<V, E>(V::Module(&ModName(\"collections.abc\"))) }}")));
                    }
                    if self.user_modules.contains(&m) {
                        return self.user_mod_load(&m, n, line);
                    }
                    if !module_has(&m, n) {
                        if hybrid_bridged(&m) {
                            self.bridged_used.insert(m.clone());
                            return Ok(self.q(&format!("pyd_from({}, {})", lit(&m), lit(n))));
                        }
                        return Err(err(line, &format!("{m}.{n} is not supported in dynamic builds")));
                    }
                    return Ok(self.q(&format!("module_attr({}, {})", lit(&m), lit(n))));
                }
                np_attr_check(n, &[], line)?;
                if let Some(why) = frame_attr_refused(n) {
                    return Err(err(line, &why));
                }
                if let Ex::Name(x) = &**o {
                    if self.types_alias.contains(&(self.module.clone(), x.clone())) {
                        lib_attr_check("types", n, line)?;
                    }
                }
                let oo = self.ex(o, line)?;
                let ic = new_ic();
                self.q(&format!("getattr_ic(&{oo}, {}, &IC{ic})", lit(n)))
            }
            Ex::Index(o, i) if self.is_globals(o) => {
                let ii = self.ex(i, line)?;
                self.q(&format!("glob_item({}, &{ii})", self.gtab()))
            }
            Ex::Index(o, i) => {
                let oo = self.ex(o, line)?;
                let ii = self.ex(i, line)?;
                self.q(&format!("getitem(&{oo}, &{ii})"))
            }
            Ex::Slice(a, b, c) => {
                let f = |me: &mut Self, x: &Option<Box<Ex>>| -> ER<String> {
                    match x {
                        Some(x) => me.ex(x, line),
                        None => Ok("V::None".into()),
                    }
                };
                let (a, b, c) = (f(self, a)?, f(self, b)?, f(self, c)?);
                format!("slice({a}, {b}, {c})")
            }
            Ex::Call(f, args) => self.call(f, args, line)?,
            Ex::Bin(op, a, b) => {
                if *op == "@" && np_on() && np_narrow() && !np_f32_simd_here() {
                    return Err(err(line, "numpy: @ with float32 arrays (OpenBLAS sgemm) is not in Brevi's native numpy"));
                }
                let aa = self.ex(a, line)?;
                let bb = self.ex(b, line)?;
                match *op {
                    "+" => self.q(&format!("add(&{aa}, &{bb})")),
                    "-" => self.q(&format!("sub(&{aa}, &{bb})")),
                    _ => self.q(&format!("binop(&{aa}, &{bb}, {})", lit(op))),
                }
            }
            Ex::Unary(op, a) => {
                let aa = self.ex(a, line)?;
                match *op {
                    "not" => {
                        let c = self.q(&format!("truth(&{aa})"));
                        format!("V::Bool(!{c})")
                    }
                    "-" => {
                        if let Ex::Int(_) | Ex::Float(_) = &**a {
                            // constant fold -literal
                            self.q(&format!("neg(&{aa})"))
                        } else {
                            self.q(&format!("neg(&{aa})"))
                        }
                    }
                    "+" => self.q(&format!("pos(&{aa})")),
                    _ => self.q(&format!("invert(&{aa})")),
                }
            }
            Ex::And(a, b) => {
                let aa = self.ex(a, line)?;
                let bb = self.ex(b, line)?;
                let t = self.q("truth(&__t)");
                format!("{{ let __t = {aa}; if {t} {{ {bb} }} else {{ __t }} }}")
            }
            Ex::Or(a, b) => {
                let aa = self.ex(a, line)?;
                let bb = self.ex(b, line)?;
                let t = self.q("truth(&__t)");
                format!("{{ let __t = {aa}; if {t} {{ __t }} else {{ {bb} }} }}")
            }
            // One comparison as a value: an object's __eq__ / __lt__ ... result is the value (polars expressions, numpy arrays).
            Ex::Cmp(a, rest) if rest.len() == 1 && !matches!(rest[0].0, "is" | "is not" | "in" | "not in") => {
                let aa = self.ex(a, line)?;
                let bb = self.ex(&rest[0].1, line)?;
                self.q(&format!("cmp_v(&{aa}, &{bb}, {})", lit(rest[0].0)))
            }
            Ex::Cmp(..) => format!("V::Bool({})", self.cond(e, line)?),
            Ex::IfExp(c, a, b) => {
                let cc = self.cond(c, line)?;
                let aa = self.ex(a, line)?;
                let bb = self.ex(b, line)?;
                format!("(if {cc} {{ {aa} }} else {{ {bb} }})")
            }
            Ex::Lambda(p, body) => {
                // a body all on one line runs at that line, even in a statement over several lines
                let bl = if p.line != 0 { p.line } else { line };
                let ret = Stmt { line: bl, st: St::Return(Some((**body).clone())) };
                let qn = self.qualname_of("<lambda>");
                let saved = std::mem::replace(&mut self.lam_line, if p.line != 0 { Some(p.line) } else { None });
                let f = self.closure("<lambda>", &qn, p, &[ret], line, false);
                self.lam_line = saved;
                let f = f?;
                let mut f = format!("with_qualname({f}, {})", lit(&qn));
                // a lambda has no annotations: its __annotations__ is {} (kept like a def's, see ANN_DEFS)
                if super::typing::ANN_DEFS.load(std::sync::atomic::Ordering::Relaxed) {
                    f = self.q(&format!("with_ann({f}, vec![])"));
                }
                if PYD_ON.load(std::sync::atomic::Ordering::Relaxed) { format!("with_np({f}, {})", if p.star.is_some() { 99 } else { p.pos.len() }) } else { f }
            }
            Ex::Comp(kind, elt, val, gens) => self.comp(*kind, elt, val.as_deref(), gens, line)?,
            Ex::Walrus(n, v) => {
                let vv = self.ex(v, line)?;
                let st = self.store(n, "__w.clone()");
                format!("{{ let __w = {vv}; {st} __w }}")
            }
            Ex::Yield(x) => {
                let xx = match x {
                    Some(x) => self.ex(x, line)?,
                    None => "V::None".into(),
                };
                if self.fstack.last().unwrap().is_agen {
                    self.q(&format!("agen_yield({xx})"))
                } else {
                    self.q(&format!("gen_yield({xx})"))
                }
            }
            Ex::YieldFrom(x) => {
                if self.fstack.last().unwrap().is_async {
                    return Err(err(line, "'yield from' inside async function"));
                }
                let xx = self.ex(x, line)?;
                self.q(&format!("gen_yield_from(&{xx})"))
            }
            Ex::Await(x) => {
                if !self.fstack.last().unwrap().is_async {
                    return Err(err(line, "'await' outside async function"));
                }
                let xx = self.ex(x, line)?;
                self.q(&format!("aw(&{xx})"))
            }
        })
    }
    fn seq(&mut self, v: &[Ex], line: usize) -> ER<String> {
        if v.iter().any(|x| matches!(x, Ex::Star(_))) {
            let mut b = String::from("{ let mut __v: Vec<V> = vec![]; ");
            for x in v {
                match x {
                    Ex::Star(inner) => {
                        let ii = self.ex(inner, line)?;
                        let tv = self.q(&format!("to_vec(&{ii})"));
                        write!(b, "__v.extend({tv}); ").unwrap();
                    }
                    _ => {
                        let xx = self.ex(x, line)?;
                        write!(b, "__v.push({xx}); ").unwrap();
                    }
                }
            }
            b.push_str("__v }");
            return Ok(b);
        }
        let mut parts = vec![];
        for x in v {
            parts.push(self.ex(x, line)?);
        }
        Ok(format!("vec![{}]", parts.join(", ")))
    }
    fn fparts(&mut self, parts: &[FPart], line: usize) -> ER<String> {
        let mut b = String::new();
        for p in parts {
            match p {
                FPart::Lit(s) => write!(b, "__s.push_str({}); ", lit(s)).unwrap(),
                FPart::Expr(e, conv, spec, _) => {
                    self.lib_repr_check(e, line)?;
                    let ee = self.ex(e, line)?;
                    let v = match conv {
                        Some('r') => format!("st({})", self.q(&format!("repr(&{ee})"))),
                        Some('s') => format!("st({})", self.q(&format!("to_str(&{ee})"))),
                        Some('a') => self.q(&format!("bi_ascii(Args::p(vec![{ee}]))")),
                        _ => ee,
                    };
                    if spec.is_empty() {
                        let t = self.q(&format!("to_str(&__x)"));
                        write!(b, "{{ let __x = {v}; if let V::Str(__y) = &__x {{ __s.push_s(__y); }} else {{ __s.push_str(&{t}); }} }} ").unwrap();
                    } else {
                        let sp = self.fparts(spec, line)?;
                        let fs = self.q("fmt_s(&__x, &__spec)");
                        write!(b, "{{ let __x = {v}; let __spec = {{ let mut __s = Txt::new(); {sp} __s.into_text() }}; __s.push_s(&{fs}); }} ").unwrap();
                    }
                }
            }
        }
        Ok(b)
    }
    // A Rust bool for a condition.
    fn cond(&mut self, e: &Ex, line: usize) -> ER<String> {
        Ok(match e {
            Ex::Cmp(a, rest) => {
                if rest.len() == 1 && np_on() && !matches!(rest[0].0, "is" | "is not" | "in" | "not in") {
                    let aa = self.ex(a, line)?;
                    let bb = self.ex(&rest[0].1, line)?;
                    let c = self.q(&format!("cmp_v(&{aa}, &{bb}, {})", lit(rest[0].0)));
                    return Ok(self.q(&format!("truth(&{c})")));
                }
                if rest.len() == 1 {
                    let aa = self.ex(a, line)?;
                    let op = rest[0].0;
                    let is_g = matches!(op, "in" | "not in") && self.is_globals(&rest[0].1);
                    let bb = if is_g { String::new() } else { self.ex(&rest[0].1, line)? };
                    return Ok(match op {
                        "is" => format!("is(&{aa}, &{bb})"),
                        "is not" => format!("!is(&{aa}, &{bb})"),
                        "==" => self.q(&format!("eq(&{aa}, &{bb})")),
                        "!=" => self.q(&format!("ne(&{aa}, &{bb})")),
                        "in" if is_g => self.q(&format!("glob_has({}, &{aa})", self.gtab())),
                        "not in" if is_g => format!("!{}", self.q(&format!("glob_has({}, &{aa})", self.gtab()))),
                        "in" => self.q(&format!("contains(&{bb}, &{aa})")),
                        "not in" => format!("!{}", self.q(&format!("contains(&{bb}, &{aa})"))),
                        _ => self.q(&format!("rich(&{aa}, &{bb}, {})", lit(op))),
                    });
                }
                let aa = self.ex(a, line)?;
                let mut b = format!("{{ let mut __l = {aa}; let mut __ok = true; ");
                for (i, (op, x)) in rest.iter().enumerate() {
                    let xx = self.ex(x, line)?;
                    let c = self.q(&format!("cmp_op(&__l, &__r, {})", lit(op)));
                    write!(b, "if __ok {{ let __r = {xx}; __ok = {c}; __l = __r; }} ").unwrap();
                    let _ = i;
                }
                b.push_str("__ok }");
                b
            }
            Ex::And(a, b) => format!("({} && {})", self.cond(a, line)?, self.cond(b, line)?),
            Ex::Or(a, b) => format!("({} || {})", self.cond(a, line)?, self.cond(b, line)?),
            Ex::Unary("not", a) => format!("!{}", self.cond(a, line)?),
            Ex::True => "true".into(),
            Ex::False => "false".into(),
            _ => {
                let ee = self.ex(e, line)?;
                self.q(&format!("truth(&{ee})"))
            }
        })
    }
    // the module name `x` refers to, when x is bound only by `import m`
    // A call of a re function, told its line first: the warnings CPython's re issues name the caller's line.
    // `call` is the call's Rust text over `__ra`, the evaluated arguments.
    fn re_call(&mut self, call: &str, args: &str, line: usize) -> String {
        // a statement over several lines: which of them the call is on is not known, so no line is named
        // (the module is still known: line 0 says so, and a warning the filters turn into an error is exact there)
        let site = match self.wmod() {
            Some(m) if self.lam_line == Some(line) || !self.multi_line(line) => format!("re_site({line}, &{m});"),
            Some(m) => format!("re_site(0, &{m});"),
            None => "re_site_clear();".to_string(),
        };
        format!("{{ let __ra = {args}; let __ws = re_site_get(); {site} let __rr = {call}; re_site_put(__ws); __rr }}")
    }
    // The static naming this module of the program for warnings (None for Brevi's own library copies).
    fn wmod(&mut self) -> Option<String> {
        let (file, src) = self.bridge.srcs.get(&self.module).cloned()?;
        let name = format!("WM_{}", rust_ident(if self.module.is_empty() { "__main__" } else { &self.module }));
        if self.wmods.insert(name.clone()) {
            let module = if self.module.is_empty() { "__main__".to_string() } else { self.module.clone() };
            write!(self.items, "static {name}: WMod = WMod {{ file: {}, module: {}, src: {} }};\n", lit(&file), lit(&module), lit(&src)).unwrap();
        }
        Some(name)
    }
    // Whether a call in the logical line starting at `line` of this module may start on a later line than it.
    // CPython names the line a call starts on; the dynamic build knows a statement's first line only, so
    // a statement over several lines is known only when every call in it opens on its first line.
    fn multi_line(&mut self, line: usize) -> bool {
        use super::parse::Tok;
        if !self.multi.contains_key(&self.module) {
            let set = self.bridge.srcs.get(&self.module).and_then(|(_, src)| super::parse::lex(src).ok()).map(|lx| {
                let mut set = HashSet::new();
                let mut start: Option<usize> = None;
                let mut prev: Option<&Tok> = None;
                for (tk, l) in &lx.toks {
                    match tk {
                        Tok::Indent | Tok::Dedent | Tok::Eof => {}
                        Tok::Newline => {
                            start = None;
                        }
                        _ => {
                            let s = *start.get_or_insert(*l);
                            // a `(` after a name, `)`, `]`, `}` or a string may be a call
                            let call = matches!(tk, Tok::Op("(")) && matches!(prev, Some(Tok::Name(_) | Tok::Op(")") | Tok::Op("]") | Tok::Op("}") | Tok::Str(..) | Tok::FStr(..) | Tok::Bytes(_)));
                            if *l != s && (call || matches!(tk, Tok::Op("@"))) {
                                set.insert(s);
                            }
                        }
                    }
                    prev = Some(tk);
                }
                set
            });
            self.multi.insert(self.module.clone(), set);
        }
        match self.multi.get(&self.module) {
            Some(Some(set)) => set.contains(&line),
            _ => true,
        }
    }
    // The statement that says where the program is (`line` of this module), for a program using warnings;
    // `known` false where the line is not known (a decorated def, an elif condition).
    fn wsite(&mut self, line: usize, known: bool) -> String {
        if !self.track_warn {
            return String::new();
        }
        let known = known && line != 0 && (self.lam_line == Some(line) || !self.multi_line(line));
        // Brevi's own library copies have no lines of CPython's to name; line 0 is this module at a line not known
        match self.wmod() {
            Some(m) => format!("re_site({}, &{m});", if known { line } else { 0 }),
            None => "re_site_clear();".into(),
        }
    }
    fn static_module(&self, e: &Ex) -> Option<String> {
        if let Ex::Name(n) = e {
            if let Some(Var::Global(_)) = self.lookup(n) {
                return self.mod_alias.get(&(self.module.clone(), n.clone())).cloned();
            }
        }
        // `json.decoder` after `import json.decoder` (or `import json`, which loads it too).
        if let Ex::Attr(o, n) = e {
            if let Some(m) = self.static_module(o) {
                let full = format!("{m}.{n}");
                if builtin_mod(full.as_str()) && !module_has(&m, n) {
                    return Some(full);
                }
            }
        }
        None
    }
    // A library module (or sys.stdin/stdout/stderr, decimal.getcontext()) whose str or repr the program asks for
    // where the build can see it (`print(math)`, `repr(m)`, `f"{math}"`): refused here rather than stopped at run
    // time (rt_fmt.rs module_repr), since its repr names where CPython's own copy is installed.
    fn lib_repr_check(&self, e: &Ex, line: usize) -> ER<()> {
        let what = match e {
            Ex::Attr(o, n) if matches!(n.as_str(), "stdin" | "stdout" | "stderr") && self.static_module(o).as_deref() == Some("sys") => Some(format!("the repr of sys.{n}")),
            Ex::Call(f, a) if a.is_empty() && matches!(&**f, Ex::Attr(o, n) if n == "getcontext" && self.static_module(o).as_deref() == Some("decimal")) => Some("the repr of decimal.getcontext()".to_string()),
            _ => match self.static_module(e) {
                Some(m) if m != "sys" && m != "builtins" => Some(format!("the repr of module {m}")),
                _ => None,
            },
        };
        match what {
            Some(w) => Err(err(line, &format!("{w} is not supported in dynamic builds"))),
            None => Ok(()),
        }
    }
    fn user_mod_load(&mut self, m: &str, n: &str, line: usize) -> ER<String> {
        if !self.mod_names.get(m).is_some_and(|s| s.contains(n)) {
            if let Some(d) = self.mod_dunder(m, n) {
                return match d {
                    Ok(v) => Ok(self.q(&format!("Ok::<V, E>({v})"))),
                    Err(()) => Err(err(line, &format!("{m}.{n} is not supported in dynamic builds"))),
                };
            }
            return Ok(self.q(&format!("Err::<V, E>(exc(\"AttributeError\", {}.into()))", lit(&format!("module '{m}' has no attribute '{n}'")))));
        }
        let i = self.gslot(m, n);
        let _ = line;
        Ok(self.q(&format!("gl({i}, {})", lit(n))))
    }
    // A module attribute CPython's import sets on a file of the program, for a name the file does not assign:
    // Ok(the V expression) for the ones Brevi gives exactly, Err for the ones it refuses (the spec, loader and
    // cached .pyc name are CPython's import machinery; a package's __path__ is its search list). None otherwise.
    fn mod_dunder(&mut self, m: &str, n: &str) -> Option<Result<String, ()>> {
        let (path, doc, pkg) = self.mod_info.get(m).cloned()?;
        let opt = |e: &mut Self, x: Option<String>| x.map(|x| e.konst(&x)).unwrap_or_else(|| "V::None".into());
        Some(match n {
            "__name__" => Ok(self.konst(m)),
            "__file__" => Ok(self.konst(&path)),
            "__doc__" => Ok(opt(self, doc)),
            "__package__" => Ok(opt(self, pkg)),
            "__spec__" | "__loader__" | "__cached__" | "__builtins__" | "__dict__" => Err(()),
            "__path__" if path.ends_with("__init__.py") => Err(()),
            _ => return None,
        })
    }
    fn args(&mut self, args: &[Arg], line: usize, prepend: Option<String>) -> ER<String> {
        let simple = args.iter().all(|a| matches!(a, Arg::Pos(_) | Arg::Kw(..)));
        let mut pos = vec![];
        if let Some(p) = prepend {
            pos.push(p);
        }
        if simple {
            let mut kw = vec![];
            for a in args {
                match a {
                    Arg::Pos(x) => pos.push(self.ex(x, line)?),
                    Arg::Kw(n, x) => kw.push(format!("({}, {})", lit(n), self.ex(x, line)?)),
                    _ => {}
                }
            }
            if kw.is_empty() {
                return Ok(format!("Args::p(vec![{}])", pos.join(", ")));
            }
            return Ok(format!("Args {{ p: vec![{}], k: vec![{}] }}", pos.join(", "), kw.join(", ")));
        }
        let mut b = format!("{{ let mut __a = Args::p(vec![{}]); ", pos.join(", "));
        for a in args {
            match a {
                Arg::Pos(x) => {
                    let xx = self.ex(x, line)?;
                    write!(b, "__a.p.push({xx}); ").unwrap();
                }
                Arg::Kw(n, x) => {
                    let xx = self.ex(x, line)?;
                    write!(b, "__a.k.push(({}, {xx})); ", lit(n)).unwrap();
                }
                Arg::Star(x) => {
                    let xx = self.ex(x, line)?;
                    let c = self.q(&format!("star_args(&mut __a, &{xx})"));
                    write!(b, "{c}; ").unwrap();
                }
                Arg::KwStar(x) => {
                    let xx = self.ex(x, line)?;
                    let c = self.q(&format!("star_kwargs(&mut __a, &{xx})"));
                    write!(b, "{c}; ").unwrap();
                }
            }
        }
        b.push_str("__a }");
        Ok(b)
    }
    // `globals()` (the builtin, not shadowed)
    fn is_globals(&self, e: &Ex) -> bool {
        matches!(e, Ex::Call(f, a) if a.is_empty() && matches!(&**f, Ex::Name(n) if n == "globals") && self.lookup("globals").is_none())
    }
    fn gtab(&self) -> String {
        format!("gtab_{}", rust_ident(&self.module))
    }
    fn call(&mut self, f: &Ex, args: &[Arg], line: usize) -> ER<String> {
        // `types.coroutine(f)` called (a method call never goes through the attribute check below)
        if let Ex::Attr(o, m) = f {
            if matches!(&**o, Ex::Name(x) if self.types_alias.contains(&(self.module.clone(), x.clone()))) {
                lib_attr_check("types", m, line)?;
            }
            if let Some(md) = self.static_module(o) {
                lib_attr_check(&md, m, line)?;
            }
        }
        // warnings.warn(..., stacklevel=2) in the program names a caller's line, which the dynamic build does
        // not keep: CPython runs it (a stacklevel it only learns at run time is refused there, see pyglue)
        if self.track_warn {
            if let Ex::Attr(o, m) = f {
                if m == "warn" && matches!(o.as_ref(), Ex::Name(n) if self.warn_names.contains(&(self.module.clone(), n.clone())) && matches!(self.lookup(n), Some(Var::Global(_)))) {
                    let lvl = args.iter().find_map(|a| match a { Arg::Kw(k, v) if k == "stacklevel" => Some(v), _ => None })
                        .or_else(|| args.iter().filter(|a| matches!(a, Arg::Pos(_))).nth(2).and_then(|a| if let Arg::Pos(v) = a { Some(v) } else { None }));
                    if let Some(v) = lvl {
                        if !matches!(v, Ex::Int(n) if n.parse::<i64>().is_ok_and(|n| n <= 1)) {
                            return Err(err(line, "warnings.warn with a stacklevel above 1 is not supported in dynamic builds"));
                        }
                    }
                }
            }
        }
        if let Ex::Name(n) = f {
            if matches!(n.as_str(), "print" | "repr" | "str" | "ascii" | "format") && self.lookup(n).is_none() {
                for (k, a) in args.iter().enumerate() {
                    if let Arg::Pos(x) = a {
                        if n == "print" || k == 0 {
                            self.lib_repr_check(x, line)?;
                        }
                    }
                }
            }
        }
        // `from re import sub` then `sub(...)`: told its line, as re.sub(...) is
        if let Ex::Name(n) = f {
            if self.re_names.contains(&(self.module.clone(), n.clone())) && matches!(self.lookup(n), Some(Var::Global(_))) {
                self.uses_re = true;
                let fv = self.ex(f, line)?;
                let a = self.args(args, line, None)?;
                let rc = self.re_call(&format!("call(&{fv}, __ra)"), &a, line);
                return Ok(self.q(&rc));
            }
        }
        // Signature.from_callable reads the parameters and annotations each def records (SIG_DEFS); a program
        // whose annotations are not kept (`from __future__ import annotations`, a type expression) is refused.
        // With CPython's inspect (the program uses more of it than Signature), its Signature cannot read a
        // dynamic build's functions either.
        if matches!(f, Ex::Attr(_, m) if m == "from_callable")
            && (!super::typing::SIG_DEFS.load(std::sync::atomic::Ordering::Relaxed) || !INSPECT_NATIVE.load(std::sync::atomic::Ordering::Relaxed))
        {
            return Err(err(line, "Signature.from_callable in a program whose def annotations are not kept is not supported in dynamic builds"));
        }
        // eval("literal text"): parsed when the program is built and run in place. Refused when a name in it
        // would resolve differently (CPython's eval sees this frame's locals and the globals, not enclosing cells).
        if let [Arg::Pos(Ex::Str(text))] = args {
            if matches!(f, Ex::Name(n) if n == "eval") && self.lookup("eval").is_none() {
                let src = text.trim_start_matches([' ', '\t']);
                match eval_numbers(src) {
                    Some(true) => {}
                    Some(false) => return Err(err(line, "eval() of a complex literal is not supported in dynamic builds")),
                    None => return Ok(self.q("Err::<V, E>(exc(\"SyntaxError\", \"invalid syntax\".to_string()))")),
                }
                let parsed = super::parse::parse(src).ok().and_then(|v| match v.as_slice() {
                    [one] => match &one.st {
                        St::Expr(e) => Some(e.clone()),
                        _ => None,
                    },
                    _ => None,
                });
                let Some(e) = parsed else {
                    return Ok(self.q("Err::<V, E>(exc(\"SyntaxError\", \"invalid syntax\".to_string()))"));
                };
                let mut names = BTreeSet::new();
                names_read_ex(&e, &mut names);
                let fr = self.fstack.last().unwrap();
                let module_vars = &self.fstack[0].vars;
                let ok = names.iter().all(|n| fr.vars.contains_key(n) || module_vars.contains_key(n) || is_builtin_name(n));
                let has_lambda = format!("{e:?}").contains("Lambda");
                if ok && !has_lambda {
                    return self.ex(&e, line);
                }
                return Err(err(line, "eval() of names this frame does not hold is not supported in dynamic builds"));
            }
        }
        // dir() of the current scope needs the frame's names as CPython orders them: refused (dir(x) is native)
        if args.is_empty() && matches!(f, Ex::Name(n) if n == "dir") && self.lookup("dir").is_none() {
            return Err(err(line, "dir() without an argument is not supported in dynamic builds"));
        }
        // vars() with no argument is locals(): the runtime's vars takes the object it reads
        if args.is_empty() && matches!(f, Ex::Name(n) if n == "vars") && self.lookup("vars").is_none() {
            return Err(err(line, "vars() without an argument is not supported in dynamic builds"));
        }
        // locals() in a function: a snapshot dict of its bound names, in CPython's order
        if args.is_empty() && matches!(f, Ex::Name(n) if n == "locals") {
            match self.lookup("locals") {
                None => {
                    let fr = self.fstack.last().unwrap();
                    // in a class body run with a namespace dict, locals() is that dict itself
                    if let Some(r) = fr.ns.clone() {
                        return Ok(format!("{r}.clone()"));
                    }
                    if !fr.is_module && !fr.is_class && !fr.is_gen && !fr.order.is_empty() || (!fr.is_module && !fr.is_class && !fr.is_gen && fr.vars.is_empty()) {
                        let mut b = String::from("{ let mut __d = Dict::new(); ");
                        // the parser's own hidden names (a match statement's subject and captures) are not the program's
                        for n in fr.order.clone().into_iter().filter(|n| !n.starts_with("__brevi_") && !n.starts_with("__bv_")) {
                            let get = match self.lookup(&n) {
                                Some(Var::Local(r)) => format!("{r}.clone()"),
                                Some(Var::Cell(r)) => format!("{r}.borrow().clone()"),
                                _ => continue,
                            };
                            let set = self.q(&format!("__d.set(s({}), __v)", lit(&n)));
                            write!(b, "{{ let __v = {get}; if !matches!(__v, V::Unbound) {{ {set}; }} }} ").unwrap();
                        }
                        b.push_str("V::Dict(Rc::new(RefCell::new(__d))) }");
                        return Ok(b);
                    }
                }
                // `locals` is a name of the class body itself: it may hold a shadowing value, or (not set
                // yet, or deleted) the builtin locals, which is this namespace dict.
                Some(Var::NS(r)) if self.f().ns.is_some() => {
                    let id = self.id();
                    let get = self.q(&format!("class_ns_get(&{r}, {})", lit("locals")));
                    let body = format!("{{ let __v{id} = {get}; if let Some(__v{id}) = __v{id} {{ call(&__v{id}, Args::p(vec![])) }} else {{ Ok({r}.clone()) }} }}");
                    return Ok(self.q(&body));
                }
                _ => {}
            }
        }
        // globals().get(k) / globals().get(k, d)
        if let Ex::Attr(o, m) = f {
            if m == "get" && self.is_globals(o) && (1..=2).contains(&args.len()) && args.iter().all(|a| matches!(a, Arg::Pos(_))) {
                let mut xs = vec![];
                for a in args {
                    if let Arg::Pos(x) = a {
                        xs.push(self.ex(x, line)?);
                    }
                }
                let d = xs.get(1).cloned().unwrap_or_else(|| "V::None".into());
                return Ok(self.q(&format!("glob_get({}, &{}, {d})", self.gtab(), xs[0])));
            }
        }
        // super().m(...) / super(C, self).m(...)
        if let Ex::Attr(o, m) = f {
            if let Ex::Call(sf, sargs) = &**o {
                if matches!(&**sf, Ex::Name(n) if n == "super") && self.lookup("super").is_none() {
                    let (cls, me) = if sargs.len() == 2 {
                        let a = match &sargs[0] {
                            Arg::Pos(x) => self.ex(x, line)?,
                            _ => return Err(err(line, "bad super() call")),
                        };
                        let b = match &sargs[1] {
                            Arg::Pos(x) => self.ex(x, line)?,
                            _ => return Err(err(line, "bad super() call")),
                        };
                        (a, b)
                    } else {
                        let fr = self.fstack.last().unwrap();
                        let cls = fr.class_expr.clone().ok_or_else(|| err(line, "super() outside a method is not supported"))?;
                        let me = fr.self_name.clone().ok_or_else(|| err(line, "super() outside a method is not supported"))?;
                        // the class's name, read so that a class still being made (its name not bound yet) is found
                        let cls = match (self.f().ns.is_none(), self.lookup(&cls)) {
                            (true, Some(Var::Global(i))) => self.q(&format!("super_cls(gl_raw({i}), {}, 0)", lit(&cls))),
                            (true, Some(Var::Cell(r))) => self.q(&format!("super_cls({r}.borrow().clone(), {}, 1)", lit(&cls))),
                            _ => self.load_raw(&cls, line)?,
                        };
                        let me = self.load(&me, line)?;
                        (cls, me)
                    };
                    let a = self.args(args, line, None)?;
                    let sa = self.q(&format!("super_attr(&{cls}, &{me}, {})", lit(m)));
                    return Ok(self.q(&format!("call(&{sa}, {a})")));
                }
            }
            // np.maximum.accumulate(x), np.add.reduce(x, axis=1), ...: numpy's ufunc methods
            if let Ex::Attr(o2, uf) = &**o {
                if np_native() && self.static_module(o2).as_deref() == Some("numpy") && matches!(m.as_str(), "reduce" | "accumulate") {
                    if np_narrow() || !matches!(uf.as_str(), "add" | "multiply" | "maximum" | "minimum" | "logical_and" | "logical_or") || (uf.starts_with("logical") && m == "accumulate") {
                        return Err(err(line, &format!("numpy: np.{uf}.{m} is not in Brevi's native numpy")));
                    }
                    for x in args {
                        match x {
                            Arg::Pos(_) => {}
                            Arg::Kw(k, _) if matches!(k.as_str(), "array" | "axis" | "keepdims") => {}
                            _ => return Err(err(line, &format!("numpy: np.{uf}.{m}(...) with these arguments is not in Brevi's native numpy"))),
                        }
                    }
                    let a = self.args(args, line, None)?;
                    return Ok(self.q(&format!("module_call(\"numpy\", {}, {a})", lit(&format!("{uf}.{m}")))));
                }
            }
            if let Some(md) = self.static_module(o) {
                if self.user_modules.contains(&md) {
                    let fv = self.user_mod_load(&md, m, line)?;
                    let a = self.args(args, line, None)?;
                    return Ok(self.q(&format!("call(&{fv}, {a})")));
                }
                // time.strftime(fmt) formats local time, which dynamic builds do not know: CPython runs it.
                if md == "time" && m == "strftime" && args.len() < 2 {
                    return Err(err(line, "time.strftime without a time tuple (local time) is not supported in dynamic builds"));
                }
                if md == "re" {
                    self.uses_re = true;
                }
                let a = self.args(args, line, None)?;
                if !module_has(&md, m) {
                    if hybrid_bridged(&md) {
                        lib_attr_check(&md, m, line)?;
                        self.bridged_used.insert(md.clone());
                        let f = self.q(&format!("pyd_from({}, {})", lit(&md), lit(m)));
                        return Ok(self.q(&format!("call(&{f}, {a})")));
                    }
                    return Err(err(line, &format!("{md}.{m} is not supported in dynamic builds")));
                }
                if md == "numpy" || md == "numpy.random" || md == "numpy.linalg" {
                    let ok = match md.as_str() {
                        "numpy" => np_func_kw(m),
                        "numpy.random" => np_rand_kw(m),
                        _ => np_linalg_kw(m),
                    }
                    .unwrap_or(&[]);
                    for x in args {
                        match x {
                            Arg::Kw(k, _) if !ok.contains(&k.as_str()) && !matches!(k.as_str(), "a" | "x" | "object" | "ar" | "q" | "v" | "shape" | "fill_value" | "dtype" | "out") || matches!(x, Arg::Kw(k, _) if k == "out") => {
                                return Err(err(line, &format!("numpy: np.{m}(..., {}=...) is not in Brevi's native numpy", if let Arg::Kw(k, _) = x { k.as_str() } else { "" })));
                            }
                            Arg::Pos(_) | Arg::Kw(..) => {}
                            _ => return Err(err(line, &format!("numpy: np.{m}(*args or **kwargs) is not in Brevi's native numpy"))),
                        }
                    }
                }
                if md == "re" {
                    let rc = self.re_call(&format!("module_call(\"re\", {}, __ra)", lit(m)), &a, line);
                    return Ok(self.q(&rc));
                }
                return Ok(self.q(&format!("module_call({}, {}, {a})", lit(&md), lit(m))));
            }
            let kws: Vec<&str> = args.iter().filter_map(|x| if let Arg::Kw(k, _) = x { Some(k.as_str()) } else { None }).collect();
            np_attr_check(m, &kws, line)?;
            let oo = self.ex(o, line)?;
            // one positional argument: list.append, set.add and the like skip the Args Vec
            if let [Arg::Pos(x)] = args {
                let xx = self.ex(x, line)?;
                return Ok(self.q(&format!("call_method1(&{oo}, {}, {xx})", lit(m))));
            }
            let a = self.args(args, line, None)?;
            return Ok(self.q(&format!("call_method(&{oo}, {}, {a})", lit(m))));
        }
        if let Ex::Name(n) = f {
            if self.lookup(n).is_none() && is_builtin_name(n) {
                if n == "super" {
                    return Err(err(line, "super() used as a value is not supported"));
                }
                let a = self.args(args, line, None)?;
                if let Some(fname) = builtin_fast(n) {
                    // *args or **kwargs to a builtin: go through call(), where CPython's checks run
                    // (duplicate keywords, keywords must be strings) before the builtin's own
                    if args.iter().all(|x| matches!(x, Arg::Pos(_) | Arg::Kw(..))) {
                        return Ok(self.q(&format!("{fname}({a})")));
                    }
                    return Ok(self.q(&format!("call(&builtin_value({}).unwrap(), {a})", lit(n))));
                }
            }
            // direct call of a nested def held in a local of this frame
            if let Some(Var::Local(r)) = self.lookup(n) {
                if let Some((dfn, dcaps, np)) = self.fstack.last().unwrap().direct_defs.get(n).cloned() {
                    if args.len() == np && args.iter().all(|a| matches!(a, Arg::Pos(_))) {
                        let mut parts: Vec<String> = dcaps.iter().map(|r| format!("{r}.clone()")).collect();
                        for a in args {
                            if let Arg::Pos(x) = a {
                                parts.push(self.ex(x, line)?);
                            }
                        }
                        return Ok(self.q(&format!("match ldr(&{r}, {}) {{ Ok(_) => {dfn}({}), Err(e) => Err(e) }}", lit(n), parts.join(", "))));
                    }
                }
            }
            // direct call of a nested def, where every cell it captures is in scope
            if let Some(Var::Cell(c)) = self.lookup(n) {
                if let Some((dfn, dcaps, np)) = self.nested_direct.get(&c).cloned() {
                    let fr = self.fstack.last().unwrap();
                    let visible = dcaps.iter().all(|r| fr.vars.values().any(|v| matches!(v, Var::Cell(x) if x == r)));
                    if visible && args.len() == np && args.iter().all(|a| matches!(a, Arg::Pos(_))) {
                        let mut parts: Vec<String> = dcaps.iter().map(|r| format!("{r}.clone()")).collect();
                        for a in args {
                            if let Arg::Pos(x) = a {
                                parts.push(self.ex(x, line)?);
                            }
                        }
                        return Ok(self.q(&format!("match cbound(&{c}, {}) {{ Ok(()) => {dfn}({}), Err(e) => Err(e) }}", lit(n), parts.join(", "))));
                    }
                }
            }
            // direct call of a module-level def
            if let Some(Var::Global(_)) = self.lookup(n) {
                let key = (self.module.clone(), n.clone());
                if let Some((rf, np, dslots)) = self.direct.get(&key).cloned() {
                    if args.iter().all(|a| matches!(a, Arg::Pos(_))) && args.len() <= np && args.len() + dslots.len() >= np {
                        let mut parts = vec![];
                        for a in args {
                            if let Arg::Pos(x) = a {
                                parts.push(self.ex(x, line)?);
                            }
                        }
                        let nd = dslots.len();
                        for k in args.len()..np {
                            // default k is the (k - (np - nd))-th default
                            let slot = dslots[k - (np - nd)];
                            parts.push(format!("gr({slot})"));
                        }
                        return Ok(self.q(&format!("{rf}({})", parts.join(", "))));
                    }
                }
            }
        }
        let ff = self.ex(f, line)?;
        let a = self.args(args, line, None)?;
        Ok(self.q(&format!("call(&{ff}, {a})")))
    }
    fn load_raw(&mut self, name: &str, line: usize) -> ER<String> {
        self.load(name, line)
    }
    // A generator expression is a lazy iterator value, not a list: only the outermost iterable is evaluated
    // here. The rest runs as a small state machine in an It::GenExp closure, so conditions, inner iterables
    // and the element see each value of the names they read when the consumer pulls (CPython semantics), and
    // any()/all() short-circuit.
    fn genexp(&mut self, elt: &Ex, gens: &[CompFor], line: usize) -> ER<String> {
        let id = self.id();
        // CPython only allows `yield` in the outermost iterable (it runs in the enclosing generator); a
        // yield anywhere in the lazy part is a syntax error, so refuse it rather than miscompile it.
        let mut lazy_yield = false;
        let mut scan = |e: &Ex, lazy_yield: &mut bool| {
            visit_ex(e, &mut |x| {
                if matches!(x, Ex::Yield(_) | Ex::YieldFrom(_)) {
                    *lazy_yield = true;
                }
            });
        };
        scan(elt, &mut lazy_yield);
        for (k, g) in gens.iter().enumerate() {
            if k > 0 {
                scan(&g.iter, &mut lazy_yield);
            }
            g.ifs.iter().for_each(|c| scan(c, &mut lazy_yield));
        }
        if lazy_yield {
            return Err(err(line, "'yield' inside generator expression"));
        }
        // Names the lazy part reads from enclosing scopes (its own targets are not free).
        let free = genexp_free(gens, elt, None);
        let mut names = BTreeSet::new();
        for g in gens {
            target_names(&g.target, &mut names);
        }
        // A target read by a scope nested in the lazy part must itself be a cell.
        let mut inner_free = BTreeSet::new();
        nested_scope_free(elt, &mut inner_free);
        for (k, g) in gens.iter().enumerate() {
            if k > 0 {
                nested_scope_free(&g.iter, &mut inner_free);
            }
            g.ifs.iter().for_each(|c| nested_scope_free(c, &mut inner_free));
        }
        inner_free.retain(|n| names.contains(n));
        // The outermost iterable (or zip pair) is evaluated now, in the enclosing scope.
        let first_zip = match self.zip2(&gens[0].target, &gens[0].iter) {
            Some((a, b, t0, t1)) if !gens[0].is_async => Some((self.ex(a, line)?, self.ex(b, line)?, t0, t1)),
            _ => None,
        };
        let first_iter = if first_zip.is_some() { String::new() } else { self.ex(&gens[0].iter, line)? };
        let first_it = match &first_zip {
            Some((za, zb, _, _)) => (
                self.q(&format!("iter(&{za})")),
                self.q(&format!("iter(&{zb})")),
                String::new(),
            ),
            None => (String::new(), String::new(), self.q(&format!("iter(&{first_iter})"))),
        };
        // zip pairs of inner levels, recognized now; their expressions are emitted inside the closure frame.
        // A target called `zip` would shadow the builtin in later clauses, so stay on the general path then.
        let mut zips: Vec<Option<(&Ex, &Ex, &Ex, &Ex)>> = Vec::with_capacity(gens.len());
        for (k, g) in gens.iter().enumerate() {
            zips.push(if k == 0 || g.is_async || names.contains("zip") { None } else { self.zip2(&g.target, &g.iter) });
        }
        // The genexp's own frame: targets are its locals, enclosing cells are captured. It behaves like a
        // nested generator function (class context and self reach the lazy part too).
        let parent = self.fstack.last().unwrap();
        let gq = self.qualname_of("<genexpr>");
        let gq_lit = lit(&gq);
        let (class_expr, self_name) = (parent.class_expr.clone(), parent.self_name.clone());
        let mut fr = Func { vars: HashMap::new(), frames: vec![], class_expr, self_name, qname: gq, exc_vars: vec![], is_module: false, is_class: false, enum_cnv: None, is_async: false, is_agen: false, is_gen: true, direct_defs: HashMap::new(), cells: format!("g{id}_"), order: vec![], ns: None };
        let mut tdecl = String::new();
        for n in &names {
            if inner_free.contains(n) {
                let c = format!("g{id}_{}", rust_ident(n));
                write!(tdecl, "let {c} = cell(); ").unwrap();
                fr.vars.insert(n.clone(), Var::Cell(c));
            } else {
                let r = format!("c{id}_{}", rust_ident(n));
                write!(tdecl, "let mut {r}: V = V::Unbound; ").unwrap();
                fr.vars.insert(n.clone(), Var::Local(r));
            }
        }
        let mut captures: Vec<(String, String)> = vec![];
        for n in &free {
            match self.lookup(n) {
                Some(Var::Cell(r)) => {
                    fr.vars.insert(n.clone(), Var::Cell(r.clone()));
                    captures.push((n.clone(), r));
                }
                // a plain local the closure cannot share, such as an enclosing comprehension's target
                // (`[x for x in xs if any(f(x) for f in fs)]`): the genexp gets its value now
                Some(Var::Local(r)) => {
                    fr.vars.insert(n.clone(), Var::Local(r.clone()));
                    captures.push((n.clone(), r));
                }
                _ => {}
            }
        }
        self.fstack.push(fr);
        let mut arms = String::new();
        let mut inner_decl = String::new();
        let mut res = Ok(());
        for (k, g) in gens.iter().enumerate() {
            let build = (|| -> ER<String> {
                let mut arm = String::new();
                let next = if k + 1 < gens.len() {
                    format!("__st{id} = {};", k + 1)
                } else {
                    let e = self.ex(elt, line)?;
                    format!("return Ok(Some({e}));")
                };
                let mut conds = String::new();
                for c in &g.ifs {
                    let cc = self.cond(c, line)?;
                    write!(conds, "if !({cc}) {{ continue; }} ").unwrap();
                }
                if k == 0 {
                    if let Some((_, _, t0, t1)) = &first_zip {
                        let a0 = self.assign(t0, &format!("__x{id}_0a"), line)?;
                        let a1 = self.assign(t1, &format!("__x{id}_0b"), line)?;
                        write!(arm, "0 => match __it{id}_0a.next()? {{ Some(__x{id}_0a) => match __it{id}_0b.next()? {{ Some(__x{id}_0b) => {{ {a0} {a1} {conds}{next} }} None => return Ok(None) }} None => return Ok(None) }}, ").unwrap();
                        return Ok(arm);
                    }
                    let asg = self.assign(&g.target, &format!("__x{id}_0"), line)?;
                    write!(arm, "0 => match __it{id}_0.next()? {{ Some(__x{id}_0) => {{ {asg} {conds}{next} }} None => return Ok(None) }}, ").unwrap();
                    return Ok(arm);
                }
                if let Some((ta, tb, t0, t1)) = zips[k] {
                    let ea = self.ex(ta, line)?;
                    let eb = self.ex(tb, line)?;
                    write!(inner_decl, "let mut __it{id}_{k}a: Option<It> = None; let mut __it{id}_{k}b: Option<It> = None; ").unwrap();
                    let a0 = self.assign(t0, &format!("__x{id}_{k}a"), line)?;
                    let a1 = self.assign(t1, &format!("__x{id}_{k}b"), line)?;
                    write!(arm, "{k} => {{ if __it{id}_{k}a.is_none() {{ __it{id}_{k}a = Some(iter(&{ea})?); __it{id}_{k}b = Some(iter(&{eb})?); }} match __it{id}_{k}a.as_mut().unwrap().next()? {{ Some(__x{id}_{k}a) => match __it{id}_{k}b.as_mut().unwrap().next()? {{ Some(__x{id}_{k}b) => {{ {a0} {a1} {conds}{next} }} None => {{ __it{id}_{k}a = None; __it{id}_{k}b = None; __st{id} = {}; }} }} None => {{ __it{id}_{k}a = None; __it{id}_{k}b = None; __st{id} = {}; }} }} }}, ", k - 1, k - 1).unwrap();
                    return Ok(arm);
                }
                let asg = self.assign(&g.target, &format!("__x{id}_{k}"), line)?;
                let it = self.ex(&g.iter, line)?;
                write!(inner_decl, "let mut __it{id}_{k}: Option<It> = None; ").unwrap();
                write!(arm, "{k} => {{ if __it{id}_{k}.is_none() {{ __it{id}_{k} = Some(iter(&{it})?); }} match __it{id}_{k}.as_mut().unwrap().next()? {{ Some(__x{id}_{k}) => {{ {asg} {conds}{next} }} None => {{ __it{id}_{k} = None; __st{id} = {}; }} }} }}, ", k - 1).unwrap();
                Ok(arm)
            })();
            match build {
                Ok(a) => arms.push_str(&a),
                Err(e) => {
                    res = Err(e);
                    break;
                }
            }
        }
        self.fstack.pop();
        res?;
        let caps: String = captures.iter().map(|(_, r)| format!("let {r} = {r}.clone(); ")).collect();
        let (a0, b0, it0) = first_it;
        let head = if a0.is_empty() {
            format!("let mut __it{id}_0 = {it0}; ")
        } else {
            format!("let mut __it{id}_0a = {a0}; let mut __it{id}_0b = {b0}; ")
        };
        Ok(format!(
            "{{ {head}{tdecl}{inner_decl}let mut __st{id}: u8 = 0; {caps}genexp_iter(sym({gq_lit}), move || loop {{ match __st{id} {{ {arms}_ => unreachable!() }} }}) }}"
        ))
    }
    fn comp(&mut self, kind: char, elt: &Ex, val: Option<&Ex>, gens: &[CompFor], line: usize) -> ER<String> {
        // `async for` clauses and `await`s: the comprehension runs in place in the async function (CPython inlines
        // it too). A generator expression with them is an async generator, which Brevi builds eagerly: refused.
        let mut awaits = gens.iter().any(|g| g.is_async);
        let mut walrus = false;
        visit_ex(&Ex::Comp(kind, Box::new(elt.clone()), val.map(|v| Box::new(v.clone())), gens.to_vec()), &mut |x| {
            if matches!(x, Ex::Await(_)) {
                awaits = true;
            }
            if matches!(x, Ex::Walrus(..)) {
                walrus = true;
            }
        });
        if walrus && self.fstack.last().unwrap().is_class {
            return Err(err(line, "assignment expression within a comprehension cannot be used in a class body"));
        }
        if awaits {
            if kind == 'g' {
                return Err(err(line, "asynchronous generator expressions are not supported in dynamic builds"));
            }
            if !self.fstack.last().unwrap().is_async {
                return Err(err(line, "asynchronous comprehension outside of an asynchronous function"));
            }
        }
        if kind == 'g' {
            return self.genexp(elt, gens, line);
        }
        // comprehension variables are fresh locals for the duration
        let id = self.id();
        let mut names = BTreeSet::new();
        for g in gens {
            target_names(&g.target, &mut names);
        }
        let saved: Vec<(String, Option<Var>)> = names.iter().map(|n| (n.clone(), self.f().vars.get(n).cloned())).collect();
        // the first iterable is evaluated in the enclosing scope
        let first_zip = match self.zip2(&gens[0].target, &gens[0].iter) {
            Some((a, b, _, _)) if !gens[0].is_async => Some((self.ex(a, line)?, self.ex(b, line)?)),
            _ => None,
        };
        let first_iter = if first_zip.is_some() { String::new() } else { self.ex(&gens[0].iter, line)? };
        // In a class body the rest of the comprehension does not see the class's names (CPython resolves them
        // past the class scope, to enclosing functions, globals and builtins), and its own variables are plain
        // locals: never class namespace stores, nor Enum auto() stores (whose `&mut` borrow the enclosing
        // class-level store already holds).
        let class_saved = if self.f().is_class {
            let fr = self.f();
            Some((std::mem::take(&mut fr.vars), fr.enum_cnv.take(), fr.ns.take()))
        } else {
            None
        };
        let res = self.comp_rest(kind, elt, val, gens, line, id, &names, first_zip, first_iter);
        if let Some((vars, cnv, ns)) = class_saved {
            let fr = self.f();
            fr.vars = vars;
            fr.enum_cnv = cnv;
            fr.ns = ns;
            return res;
        }
        for (n, v) in saved {
            match v {
                Some(v) => {
                    self.f().vars.insert(n, v);
                }
                None => {
                    self.f().vars.remove(&n);
                }
            }
        }
        res
    }
    #[allow(clippy::too_many_arguments)]
    fn comp_rest(&mut self, kind: char, elt: &Ex, val: Option<&Ex>, gens: &[CompFor], line: usize, id: usize, names: &BTreeSet<String>, first_zip: Option<(String, String)>, first_iter: String) -> ER<String> {
        let mut decl = String::new();
        for n in names {
            let r = format!("c{id}_{}", rust_ident(n));
            write!(decl, "let mut {r}: V = V::Unbound; ").unwrap();
            self.f().vars.insert(n.clone(), Var::Local(r));
        }
        let mut b = String::new();
        let acc = match kind {
            'l' | 'g' => "let mut __out: Vec<V> = vec![]; ",
            's' => "let mut __out = PSet::new(); ",
            _ => "let mut __out = Dict::new(); ",
        };
        write!(b, "{{ {acc}{decl}").unwrap();
        let mut close = String::new();
        for (k, g) in gens.iter().enumerate() {
            let lbl = format!("'cl{id}_{k}");
            let zipped = match (k, &first_zip, self.zip2(&g.target, &g.iter)) {
                (_, _, _) if g.is_async => None,
                (0, Some((a, b)), Some((_, _, t0, t1))) => Some((a.clone(), b.clone(), t0, t1)),
                (0, None, _) | (_, _, None) => None,
                (_, _, Some((a, b, t0, t1))) => Some((self.ex(a, line)?, self.ex(b, line)?, t0, t1)),
            };
            if let Some((za, zb, t0, t1)) = zipped {
                let (ia, ib) = (self.q(&format!("iter(&{za})")), self.q(&format!("iter(&{zb})")));
                let (na, nb) = (self.q(&format!("__it{k}a.next()")), self.q(&format!("__it{k}b.next()")));
                write!(b, "let mut __it{k}a = {ia}; let mut __it{k}b = {ib}; {lbl}: while let Some(__x{k}a) = {na} {{ let Some(__x{k}b) = {nb} else {{ break {lbl}; }}; ").unwrap();
                let a0 = self.assign(t0, &format!("__x{k}a"), line)?;
                let a1 = self.assign(t1, &format!("__x{k}b"), line)?;
                write!(b, "{a0} {a1} ").unwrap();
                for c in &g.ifs {
                    let cc = self.cond(c, line)?;
                    write!(b, "if !({cc}) {{ continue {lbl}; }} ").unwrap();
                }
                close.push_str("} ");
                continue;
            }
            let it = if k == 0 { first_iter.clone() } else { self.ex(&g.iter, line)? };
            let (itv, nx) = if g.is_async {
                (self.q(&format!("bi_aiter(Args::p(vec![{it}]))")), self.q(&format!("afor_next(&__it{k})")))
            } else {
                (self.q(&format!("iter(&{it})")), self.q(&format!("__it{k}.next()")))
            };
            write!(b, "let mut __it{k} = {itv}; {lbl}: while let Some(__x{k}) = {nx} {{ ").unwrap();
            let asg = self.assign(&g.target, &format!("__x{k}"), line)?;
            b.push_str(&asg);
            for c in &g.ifs {
                let cc = self.cond(c, line)?;
                write!(b, "if !({cc}) {{ continue {lbl}; }} ").unwrap();
            }
            close.push_str("} ");
        }
        match kind {
            'l' | 'g' => {
                let e = self.ex(elt, line)?;
                write!(b, "__out.push({e}); ").unwrap();
            }
            's' => {
                let e = self.ex(elt, line)?;
                let a = self.q(&format!("__out.add({e})"));
                write!(b, "{a}; ").unwrap();
            }
            _ => {
                let k = self.ex(elt, line)?;
                let v = self.ex(val.unwrap(), line)?;
                let a = self.q(&format!("__out.set(__k, {v})"));
                write!(b, "let __k = {k}; {a}; ").unwrap();
            }
        }
        b.push_str(&close);
        match kind {
            'l' => b.push_str("list(__out) }"),
            'g' => b.push_str("vec_iter(ItKind::Generator, __out) }"),
            's' => b.push_str("V::Set(Rc::new(RefCell::new(__out))) }"),
            _ => b.push_str("V::Dict(Rc::new(RefCell::new(__out))) }"),
        }
        Ok(b)
    }

    // ---- assignment ----
    fn assign(&mut self, t: &Ex, val: &str, line: usize) -> ER<String> {
        Ok(match t {
            Ex::Name(n) => self.store(n, val),
            Ex::Attr(o, n) => {
                if let Some(m) = self.static_module(o) {
                    if self.user_modules.contains(&m) {
                        let i = self.gslot(&m, n);
                        return Ok(format!("gs({i}, {val});"));
                    }
                }
                let oo = self.ex(o, line)?;
                let ic = new_ic();
                let c = self.q(&format!("setattr_ic(&{oo}, {}, __v, &IC{ic})", lit(n)));
                format!("{{ let __v = {val}; {c}; }}")
            }
            Ex::Index(o, i) => {
                let oo = self.ex(o, line)?;
                let ii = self.ex(i, line)?;
                let c = self.q(&format!("setitem(&__o, &__i, __v)"));
                format!("{{ let __v = {val}; let __o = {oo}; let __i = {ii}; {c}; }}")
            }
            Ex::Tuple(v) | Ex::List(v) => {
                let id = self.id();
                let star = v.iter().position(|x| matches!(x, Ex::Star(_)));
                if star.is_none() && v.len() == 2 {
                    let un = self.q(&format!("unpack2(__u{id})"));
                    let a0 = self.assign(&v[0], &format!("__p{id}.0"), line)?;
                    let a1 = self.assign(&v[1], &format!("__p{id}.1"), line)?;
                    return Ok(format!("{{ let __u{id} = {val}; let __p{id} = {un}; {a0} {a1} }}"));
                }
                let un = match star {
                    None => self.q(&format!("unpack(&__u{id}, {})", v.len())),
                    Some(k) => self.q(&format!("unpack_star(&__u{id}, {k}, {})", v.len() - k - 1)),
                };
                let mut b = format!("{{ let __u{id} = {val}; let mut __p{id} = {un}.into_iter(); ");
                for x in v {
                    let x = match x {
                        Ex::Star(inner) => &**inner,
                        x => x,
                    };
                    let a = self.assign(x, &format!("__p{id}.next().unwrap()"), line)?;
                    b.push_str(&a);
                    b.push(' ');
                }
                b.push('}');
                b
            }
            _ => return Err(err(line, "cannot assign to this expression")),
        })
    }

    // ---- statements ----
    fn block(&mut self, body: &[Stmt]) -> ER<String> {
        let mut b = String::new();
        if self.has_del {
            // a loop body starts here too, so what rebinding the loop target freed is finalized before the body
            b.push_str("fin_point();\n");
        }
        for s in body {
            let w = self.stmt_site(s);
            b.push_str(&w);
            b.push_str(&self.stmt(s)?);
            // A program with __del__ runs the finalizers its statement made waiting (rt_core's fin_point).
            if self.has_del && matches!(s.st, St::Expr(_) | St::Assign(..) | St::AugAssign(..) | St::Del(_)) {
                b.push_str(" fin_point();");
            }
            b.push('\n');
        }
        Ok(b)
    }
    // Where statement `s` is, said before it runs (a program using warnings).
    // An annotation with no value still evaluates its target's object: `o.a: T` evaluates o, and `o[i]: T`
    // evaluates o and i (CPython's compiler_annassign), with no attribute or item read; a name binds nothing.
    // With Brevi's pydantic: whether to evaluate this annotation as CPython does (false: a model's, which its
    // descriptor statement evaluates instead), or Err when it reads a descriptor or the pydantic copy, whose
    // values are not CPython's typing objects.
    fn pyd_ann(&self, ann: &Ex, line: usize, in_class: bool) -> Result<bool, (usize, String)> {
        if !PYD_ON.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(true);
        }
        let g = PYD_ANN.lock().unwrap();
        let Some((lines, names)) = g.as_ref().and_then(|m| m.get(&self.module)) else { return Ok(true) };
        if in_class && lines.contains(&line) {
            return Ok(false);
        }
        // A bare name evaluates to what it holds whatever that is; inside a subscript, `|`, call or attribute, a
        // descriptor would be checked as a typing value it is not.
        if matches!(ann, Ex::Name(_)) {
            return Ok(true);
        }
        let mut hit = None;
        crate::typed_guard::walk_ex(ann, &mut |e| {
            if let Ex::Name(n) = e {
                if hit.is_none() && names.contains(n) {
                    hit = Some(n.clone());
                }
            }
        });
        match hit {
            Some(n) => Err(err(line, &format!("an annotation outside a model that subscripts or combines `{n}` (Brevi's pydantic holds no typing value there to evaluate) is not supported in dynamic builds"))),
            None => Ok(true),
        }
    }

    fn ann_target_only(&mut self, t: &Ex, line: usize) -> Result<String, (usize, String)> {
        Ok(match t {
            Ex::Attr(o, _) => format!("{{ let _ = {}; }}", self.ex(o, line)?),
            Ex::Index(o, i) => {
                let oo = self.ex(o, line)?;
                let ii = self.ex(i, line)?;
                format!("{{ let _ = {oo}; let _ = {ii}; }}")
            }
            _ => String::new(),
        })
    }
    fn stmt_site(&mut self, s: &Stmt) -> String {
        if !self.track_warn {
            return String::new();
        }
        let known = match &s.st {
            St::Def(d) => d.decorators.is_empty(),
            St::Class(c) => c.decorators.is_empty(),
            _ => true,
        };
        self.wsite(s.line, known)
    }
    fn loop_label(&self) -> Option<String> {
        for fr in self.fstack.last().unwrap().frames.iter().rev() {
            if let Frame::Loop(l) = fr {
                return Some(l.clone());
            }
        }
        None
    }
    // break/continue/return crossing finally frames
    fn jump(&mut self, kind: &str, val: &str, line: usize) -> ER<String> {
        let frames = self.fstack.last().unwrap().frames.clone();
        for fr in frames.iter().rev() {
            match fr {
                Frame::Fin(l) => {
                    return Ok(match kind {
                        "return" => format!("break {l} Flow::Ret({val});"),
                        "break" => format!("break {l} Flow::Brk;"),
                        _ => format!("break {l} Flow::Cont;"),
                    });
                }
                Frame::Loop(l) => {
                    if kind != "return" {
                        return Ok(format!("{kind} {l};"));
                    }
                }
                Frame::Exc(_) => {}
            }
        }
        match kind {
            "return" => Ok(format!("return Ok({val});")),
            _ => Err(err(line, &format!("'{kind}' outside loop"))),
        }
    }
    fn stmt(&mut self, s: &Stmt) -> ER<String> {
        let line = s.line;
        Ok(match &s.st {
            St::Pass => String::new(),
            St::Expr(e) => {
                if let Ex::Str(_) = e {
                    return Ok(String::new());
                }
                let ee = self.ex(e, line)?;
                format!("let _ = {ee};")
            }
            St::Assign(ts, v) => {
                let vv = self.ex(v, line)?;
                if ts.len() == 1 {
                    if let Ex::Name(_) = &ts[0] {
                        return self.assign(&ts[0], &vv, line);
                    }
                    let a = self.assign(&ts[0], "__val", line)?;
                    return Ok(format!("{{ let __val = {vv}; {a} }}"));
                }
                let mut b = format!("{{ let __val = {vv}; ");
                for t in ts {
                    b.push_str(&self.assign(t, "__val.clone()", line)?);
                }
                b.push('}');
                b
            }
            St::AnnAssign(t, v, ann, _) => {
                let mut code = match v {
                    Some(v) => {
                        let vv = self.ex(v, line)?;
                        self.assign(t, &vv, line)?
                    }
                    None => self.ann_target_only(t, line)?,
                };
                // CPython (compiler_annassign) evaluates a module or class body's annotation after the value is
                // stored (a function body's never); the class bodies that keep __annotations__ do it themselves
                let fr = self.fstack.last().unwrap();
                if self.eval_ann && (fr.is_module || fr.is_class) && self.pyd_ann(ann, line, fr.is_class)? {
                    let av = self.ex(ann, line)?;
                    code.push_str(&format!(" let _ = {av};"));
                }
                code
            }
            St::AugAssign(t, op, v) => {
                if *op == "@" && np_on() && np_narrow() && !np_f32_simd_here() {
                    return Err(err(line, "numpy: @= with float32 arrays (OpenBLAS sgemm) is not in Brevi's native numpy"));
                }
                let vv = self.ex(v, line)?;
                match t {
                    Ex::Name(n) => {
                        let cur = self.load(n, line)?;
                        let r = self.q(&format!("iop(&__c, &__r, {})", lit(op)));
                        let st = self.store(n, "__n");
                        format!("{{ let __r = {vv}; let __c = {cur}; let __n = {r}; {st} }}")
                    }
                    Ex::Attr(o, n) => {
                        let oo = self.ex(o, line)?;
                        let ic = new_ic();
                        let g = self.q(&format!("getattr_ic(&__o, {}, &IC{ic})", lit(n)));
                        let r = self.q(&format!("iop(&__c, &__r, {})", lit(op)));
                        let st = self.q(&format!("setattr_ic(&__o, {}, __n, &IC{ic})", lit(n)));
                        format!("{{ let __o = {oo}; let __c = {g}; let __r = {vv}; let __n = {r}; {st}; }}")
                    }
                    Ex::Index(o, i) => {
                        let oo = self.ex(o, line)?;
                        let ii = self.ex(i, line)?;
                        let g = self.q("getitem(&__o, &__i)");
                        let r = self.q(&format!("iop(&__c, &__r, {})", lit(op)));
                        let st = self.q("setitem(&__o, &__i, __n)");
                        format!("{{ let __o = {oo}; let __i = {ii}; let __c = {g}; let __r = {vv}; let __n = {r}; {st}; }}")
                    }
                    _ => return Err(err(line, "bad augmented assignment target")),
                }
            }
            St::Return(v) => {
                if self.fstack.last().unwrap().is_module {
                    return Err(err(line, "'return' outside function"));
                }
                if self.fstack.last().unwrap().is_agen && v.is_some() {
                    return Err(err(line, "'return' with value in async generator"));
                }
                let vv = match v {
                    Some(v) => self.ex(v, line)?,
                    None => "V::None".into(),
                };
                let j = self.jump("return", "__rv", line)?;
                format!("{{ let __rv = {vv}; {j} }}")
            }
            St::Break => self.jump("break", "", line)?,
            St::Continue => self.jump("continue", "", line)?,
            St::Raise(e, cause) => match e {
                None => {
                    let cur = self.fstack.last().unwrap().exc_vars.last().cloned();
                    match cur {
                        Some(v) => self.raise_code(&format!("E({v}.clone())")) + ";",
                        None => self.raise_code("exc(\"RuntimeError\", \"No active exception to reraise\".into())") + ";",
                    }
                }
                Some(e) => {
                    let ee = self.ex(e, line)?;
                    let mk = self.q(&format!("make_exc({ee})"));
                    let r = self.raise_code("__e");
                    let r = format!("set_context(&__e); {r}");
                    match cause {
                        // `raise X from Y`: Y (an instance, a class made one, or None) is X.__cause__
                        Some(c) => {
                            let cc = self.ex(c, line)?;
                            let sc = self.q(&format!("set_cause(&__e, {cc})"));
                            format!("{{ let __e = {mk}; {sc}; {r}; }}")
                        }
                        None => format!("{{ let __e = {mk}; {r}; }}"),
                    }
                }
            },
            St::Global(_) | St::Nonlocal(_) => String::new(),
            St::Del(ts) => {
                let mut b = String::new();
                // a class body's `del x` is DELETE_NAME: it sees only the class namespace, so an unbound
                // (or enclosing-scope) name raises NameError, never UnboundLocalError or a free-variable error
                let in_class = self.f().is_class;
                for t in ts {
                    match t {
                        Ex::Name(n) => match self.lookup(&n) {
                            Some(Var::Local(r)) => {
                                let (cls, msg) = if in_class {
                                    ("NameError", format!("name '{n}' is not defined"))
                                } else {
                                    ("UnboundLocalError", format!("cannot access local variable '{n}' where it is not associated with a value"))
                                };
                                let raise = self.raise_code(&format!("exc({}, {}.into())", lit(cls), lit(&msg)));
                                write!(b, "if matches!({r}, V::Unbound) {{ {raise} }} {r} = V::Unbound; ").unwrap();
                            }
                            Some(Var::Cell(r)) => {
                                if in_class {
                                    let msg = format!("name '{n}' is not defined");
                                    let raise = self.raise_code(&format!("exc(\"NameError\", {}.into())", lit(&msg)));
                                    write!(b, "{raise} ").unwrap();
                                } else {
                                    // a cell this function owns is a local here; one from an outer function is a free variable
                                    let own = !self.f().cells.is_empty() && r.starts_with(&self.f().cells);
                                    let (cls, msg) = if own {
                                        ("UnboundLocalError", format!("cannot access local variable '{n}' where it is not associated with a value"))
                                    } else {
                                        ("NameError", format!("cannot access free variable '{n}' where it is not associated with a value in enclosing scope"))
                                    };
                                    let raise = self.raise_code(&format!("exc({}, {}.into())", lit(cls), lit(&msg)));
                                    write!(b, "if matches!(&*{r}.borrow(), V::Unbound) {{ {raise} }} cset(&{r}, V::Unbound); ").unwrap();
                                }
                            }
                            Some(Var::Global(i)) => {
                                let c = self.q(&format!("gdel({i}, {})", lit(&n)));
                                write!(b, "{c}; ").unwrap();
                            }
                            Some(Var::NS(r)) => {
                                let c = self.q(&format!("class_ns_del(&{r}, {})", lit(&n)));
                                write!(b, "{c}; ").unwrap();
                            }
                            None => return Err(err(line, "del of an unknown name")),
                        },
                        Ex::Index(o, i) => {
                            let oo = self.ex(o, line)?;
                            let ii = self.ex(i, line)?;
                            let c = self.q(&format!("delitem(&{oo}, &{ii})"));
                            write!(b, "{c}; ").unwrap();
                        }
                        Ex::Attr(o, n) => {
                            let oo = self.ex(o, line)?;
                            let c = self.q(&format!("delattr(&{oo}, {})", lit(n)));
                            write!(b, "{c}; ").unwrap();
                        }
                        Ex::Tuple(v) | Ex::List(v) => {
                            for x in v {
                                b.push_str(&self.stmt(&Stmt { line, st: St::Del(vec![x.clone()]) })?);
                            }
                        }
                        _ => return Err(err(line, "bad del target")),
                    }
                }
                b
            }
            St::Assert(c, m) => {
                let cc = self.cond(c, line)?;
                let msg = match m {
                    Some(m) => {
                        let mm = self.ex(m, line)?;
                        format!("exc_v(\"AssertionError\", {mm})")
                    }
                    None => "exc0(\"AssertionError\")".into(),
                };
                let r = self.raise_code(&msg);
                format!("if !({cc}) {{ {r}; }}")
            }
            St::Import(v) => {
                let mut b = String::new();
                for (m, a) in v {
                    let top = m.split('.').next().unwrap().to_string();
                    let bind = a.clone().unwrap_or(top.clone());
                    let target = if a.is_some() { m.clone() } else { top.clone() };
                    if target == "warnings" {
                        self.warn_names.insert((self.module.clone(), bind.clone()));
                    }
                    let id = self.id();
                    // `import a.b` binds `a` but CPython registers `a.b` in sys.modules too
                    if target != *m && (self.user_modules.contains(m) || builtin_mod(m.as_str()) || m == "os.path") {
                        if m == "collections.abc" && self.user_modules.contains(m) {
                            let init = self.q(&format!("init_{}()", rust_ident(m)));
                            write!(b, "{init}; ").unwrap();
                        }
                        let reg = self.q(&format!("sys_modules_set({}, &V::Module(&ModName({})))", lit(m), lit(m)));
                        write!(b, "{reg}; ").unwrap();
                    }
                    if self.user_modules.contains(&target) {
                        let init = self.q(&format!("init_{}()", rust_ident(&target)));
                        let v = format!("V::Module(&ModName({}))", lit(&target));
                        let reg = self.q(&format!("sys_modules_set({}, &__m{id})", lit(&target)));
                        write!(b, "let __m{id} = {v}; {reg}; {init}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("__m{id}")));
                    } else if self.bridge.has(m) {
                        self.bridged_used.insert(m.split('.').next().unwrap_or(m).to_string());
                        let v = self.q(&format!("pyd_import({}, {})", lit(m), a.is_none()));
                        let reg = self.q(&format!("sys_modules_set({}, &__m{id})", lit(&target)));
                        write!(b, "let __m{id} = {v}; {reg}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("__m{id}")));
                    } else if builtin_mod(target.as_str()) || matches!(target.as_str(), "os.path") {
                        let v = format!("V::Module(&ModName({}))", lit(&target));
                        let reg = self.q(&format!("sys_modules_set({}, &__m{id})", lit(&target)));
                        write!(b, "let __m{id} = {v}; {reg}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("__m{id}")));
                    } else {
                        return Err(err(line, &format!("module '{m}' is not supported in dynamic builds")));
                    }
                }
                b
            }
            St::ImportFrom(m, names) => {
                let mut b = String::new();
                if m == "__future__" {
                    return Ok(b);
                }
                // typing.ByteString stays refused (see Ex::Attr above); collections.abc's warns as CPython's does
                if m == "typing" && names.iter().any(|(n, _)| n == "ByteString") && self.bridge.srcs.contains_key(&self.module) {
                    return Err(err(line, &format!("{m}.ByteString is not supported in dynamic builds")));
                }
                let user = self.user_modules.contains(m);
                // Brevi's own `collections` module reads the native classes through its own name
                // (`from collections import deque` in collections.py itself): as a builtin import.
                let self_imp = user && *m == self.module && builtin_mod(m.as_str());
                if !user && !builtin_mod(m.as_str()) && self.bridge.has(m) {
                    self.bridged_used.insert(m.split('.').next().unwrap_or(m).to_string());
                    for (n, a) in names {
                        if n == "*" {
                            return Err(err(line, "from ... import * is not supported for library modules"));
                        }
                        lib_attr_check(m, n, line)?;
                        let v = self.q(&format!("pyd_from({}, {})", lit(m), lit(n)));
                        b.push_str(&self.store(a.as_ref().unwrap_or(n), &v));
                    }
                    return Ok(b);
                }
                if user && !self_imp {
                    let init = self.q(&format!("init_{}()", rust_ident(m)));
                    let reg = self.q(&format!("sys_modules_set({}, &V::Module(&ModName({})))", lit(m), lit(m)));
                    write!(b, "{reg}; {init}; ").unwrap();
                } else if !builtin_mod(m.as_str()) && !self_imp {
                    return Err(err(line, &format!("module '{m}' is not supported in dynamic builds")));
                } else {
                    let reg = self.q(&format!("sys_modules_set({}, &V::Module(&ModName({})))", lit(m), lit(m)));
                    write!(b, "{reg}; ").unwrap();
                }
                for (n, a) in names {
                    if n == "*" {
                        if m == "collections.abc" {
                            return Err(err(line, "from collections.abc import * is not supported in dynamic builds"));
                        }
                        if user {
                            let all: Vec<String> = self.mod_names.get(m).cloned().unwrap_or_default().into_iter().filter(|x| !x.starts_with('_')).collect();
                            for x in all {
                                let v = self.user_mod_load(m, &x, line)?;
                                b.push_str(&self.store(&x, &v));
                            }
                            continue;
                        }
                        return Err(err(line, "from ... import * is not supported for library modules"));
                    }
                    let bind = a.clone().unwrap_or(n.clone());
                    if m == "collections" && n == "abc" && self.user_modules.iter().any(|u| u == "collections.abc") {
                        let sub = "collections.abc";
                        let init = self.q(&format!("init_{}()", rust_ident(sub)));
                        let reg = self.q(&format!("sys_modules_set({}, &V::Module(&ModName({})))", lit(sub), lit(sub)));
                        write!(b, "{reg}; {init}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("V::Module(&ModName({}))", lit(sub))));
                        continue;
                    }
                    if m == "re" {
                        self.re_names.insert((self.module.clone(), bind.clone()));
                    }
                    if !user && !module_has(m, n) {
                        if hybrid_bridged(m) {
                            lib_attr_check(m, n, line)?;
                            self.bridged_used.insert(m.to_string());
                            let v = self.q(&format!("pyd_from({}, {})", lit(m), lit(n)));
                            b.push_str(&self.store(&bind, &v));
                            continue;
                        }
                        return Err(err(line, &format!("{m}.{n} is not supported in dynamic builds")));
                    }
                    let sub = format!("{m}.{n}");
                    if user && self.user_modules.contains(&sub) && (self.module == *m || !self.mod_names.get(m).is_some_and(|s| s.contains(n))) {
                        // `from pkg import sub`: the package's module sub
                        let init = self.q(&format!("init_{}()", rust_ident(&sub)));
                        let reg = self.q(&format!("sys_modules_set({}, &V::Module(&ModName({})))", lit(&sub), lit(&sub)));
                        write!(b, "{reg}; {init}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("V::Module(&ModName({}))", lit(&sub))));
                        continue;
                    }
                    let v = if self_imp {
                        if !module_has(m, n) {
                            return Err(err(line, &format!("{m}.{n} is not supported in dynamic builds")));
                        }
                        self.q(&format!("module_attr({}, {})", lit(m), lit(n)))
                    } else if user {
                        self.user_mod_load(m, n, line)?
                    } else {
                        self.q(&format!("module_attr({}, {})", lit(m), lit(n)))
                    };
                    b.push_str(&self.store(&bind, &v));
                }
                b
            }
            St::If(arms, els) if arms.len() == 1 && els.is_none() && super::typing::type_checking(&arms[0].0) => String::new(),
            St::If(arms, els) => {
                let mut b = String::new();
                for (k, (c, body)) in arms.iter().enumerate() {
                    let mut cc = self.cond(c, line)?;
                    if k > 0 && self.track_warn {
                        // an elif's own line is not kept
                        let ws = self.wsite(0, false);
                        cc = format!("{{ {ws} {cc} }}");
                    }
                    let bb = self.block(body)?;
                    if k > 0 {
                        b.push_str(" else ");
                    }
                    write!(b, "if {cc} {{\n{bb}}}").unwrap();
                }
                if let Some(e) = els {
                    let bb = self.block(e)?;
                    write!(b, " else {{\n{bb}}}").unwrap();
                }
                b
            }
            St::While(c, body, els) => {
                let id = self.id();
                let lbl = format!("'l{id}");
                let mut cc = self.cond(c, line)?;
                if self.track_warn {
                    // the condition runs again after the body, at the while's line
                    let w = self.wsite(line, true);
                    cc = format!("{{ {w} {cc} }}");
                }
                self.f().frames.push(Frame::Loop(lbl.clone()));
                let bb = self.block(body)?;
                self.f().frames.pop();
                match els {
                    None => format!("{lbl}: while {cc} {{\n{bb}}}"),
                    Some(e) => {
                        let eb = self.block(e)?;
                        let bb2 = bb.replace(&format!("break {lbl};"), &format!("{{ __brk{id} = true; break {lbl}; }}"));
                        format!("{{ let mut __brk{id} = false; {lbl}: while {cc} {{\n{bb2}}} if !__brk{id} {{ {eb} }} }}")
                    }
                }
            }
            St::For(t, it, body, els) => self.for_stmt(t, it, body, els.as_deref(), line)?,
            St::Try(body, handlers, els, fin) => self.try_stmt(body, handlers, els.as_deref(), fin.as_deref(), line)?,
            St::With(items, body) => self.with_stmt(items, body, line)?,
            St::Def(d) => {
                let f = self.def(d)?;
                let st = self.store(&d.name, &f);
                match self.lookup(&d.name) {
                    Some(Var::Cell(c)) if self.guards.contains(&c) => format!("{st} __cg_{c}.mark();"),
                    _ => st,
                }
            }
            St::Class(c) => {
                if let Some(src) = self.bridge.classes.get(&(self.module.clone(), c.line)).cloned() {
                    let m = self.module.clone();
                    let imports = self.bridge.imports.get(&m).cloned().unwrap_or_default();
                    let file = self.bridge.files.get(&m).cloned().unwrap_or_default();
                    let v = self.q(&format!("pyd_class({}, {}, {}, {}, {})", lit(&m), lit(&imports), lit(&src), lit(&c.name), lit(&file)));
                    return Ok(self.store(&c.name, &v));
                }
                        let v = self.class(c)?;
                self.store(&c.name, &v)
            }
        })
    }
    // `for a, b in zip(xs, ys)` with the builtin zip: the two iterators are walked side by side without
    // making a tuple per step (zip's order: next of xs, then of ys, stopping at the first that ends)
    fn zip2<'a>(&self, t: &'a Ex, it: &'a Ex) -> Option<(&'a Ex, &'a Ex, &'a Ex, &'a Ex)> {
        let (Ex::Tuple(ts) | Ex::List(ts)) = t else { return None };
        let Ex::Call(f, args) = it else { return None };
        if ts.len() != 2 || ts.iter().any(|x| matches!(x, Ex::Star(_))) || args.len() != 2 || !matches!(&**f, Ex::Name(n) if n == "zip") || self.lookup("zip").is_some() {
            return None;
        }
        match (&args[0], &args[1]) {
            (Arg::Pos(a), Arg::Pos(b)) => Some((a, b, &ts[0], &ts[1])),
            _ => None,
        }
    }
    fn for_stmt(&mut self, t: &Ex, it: &Ex, body: &[Stmt], els: Option<&[Stmt]>, line: usize) -> ER<String> {
        let id = self.id();
        let lbl = format!("'l{id}");
        let brk = els.is_some();
        // for i in range(...) with plain ints
        let mut head = String::new();
        let mut fast = false;
        if let Ex::Call(f, args) = it {
            if matches!(&**f, Ex::Name(n) if n == "range") && self.lookup("range").is_none() && (1..=2).contains(&args.len()) && args.iter().all(|a| matches!(a, Arg::Pos(_))) {
                let mut vals = vec![];
                for a in args {
                    if let Arg::Pos(x) = a {
                        let xx = self.ex(x, line)?;
                        vals.push(self.q(&format!("range_int(&{xx})")));
                    }
                }
                let (a, b) = if vals.len() == 1 { ("0i64".to_string(), vals[0].clone()) } else { (vals[0].clone(), vals[1].clone()) };
                head = format!("let __a{id}: i64 = {a}; let __b{id}: i64 = {b}; let mut __i{id} = __a{id}; {lbl}: while __i{id} < __b{id} {{ let __x{id} = V::Int(__i{id}); __i{id} += 1; ");
                fast = true;
            }
        }
        let mut asg = String::new();
        if let (false, Some((a, b, t0, t1))) = (fast, self.zip2(t, it)) {
            let (av, bv) = (self.ex(a, line)?, self.ex(b, line)?);
            let (ia, ib) = (self.q(&format!("iter(&{av})")), self.q(&format!("iter(&{bv})")));
            let (na, nb) = (self.q(&format!("__it{id}a.next()")), self.q(&format!("__it{id}b.next()")));
            head = format!("let mut __it{id}a = {ia}; let mut __it{id}b = {ib}; {lbl}: while let Some(__x{id}a) = {na} {{ let Some(__x{id}b) = {nb} else {{ break {lbl}; }}; ");
            asg = format!("{} {}", self.assign(t0, &format!("__x{id}a"), line)?, self.assign(t1, &format!("__x{id}b"), line)?);
            fast = true;
        }
        if !fast {
            let itv = self.ex(it, line)?;
            let iv = self.q(&format!("iter(&{itv})"));
            let w = self.wsite(line, true);
            let nx = self.q(&format!("{{ {w} __it{id}.next() }}"));
            head = format!("let mut __it{id} = {iv}; {lbl}: while let Some(__x{id}) = {nx} {{ ");
        }
        if asg.is_empty() {
            asg = self.assign(t, &format!("__x{id}"), line)?;
        }
        self.f().frames.push(Frame::Loop(lbl.clone()));
        let bb = self.block(body)?;
        self.f().frames.pop();
        if !brk {
            return Ok(format!("{{ {head}{asg}\n{bb}}} }}"));
        }
        // for/else: else runs when the loop ends without break
        let eb = self.block(els.unwrap())?;
        let bb2 = bb.replace(&format!("break {lbl};"), &format!("{{ __brk{id} = true; break {lbl}; }}"));
        Ok(format!("{{ let mut __brk{id} = false; {head}{asg}\n{bb2}}} if !__brk{id} {{ {eb} }} }}"))
    }
    fn try_stmt(&mut self, body: &[Stmt], handlers: &[Handler], els: Option<&[Stmt]>, fin: Option<&[Stmt]>, line: usize) -> ER<String> {
        if let Some(fin) = fin {
            let id = self.id();
            let lbl = format!("'f{id}");
            self.f().frames.push(Frame::Fin(lbl.clone()));
            let inner = if handlers.is_empty() && els.is_none() { self.block(body)? } else { self.try_stmt(body, handlers, els, None, line)? };
            self.f().frames.pop();
            let fb = self.block(fin)?;
            let after = self.flow_dispatch(id, line)?;
            return Ok(format!("{{ let __fl{id}: Flow = {lbl}: {{ {inner}\nFlow::Normal }};\n{fb}\n{after} }}"));
        }
        let id = self.id();
        let lbl = format!("'t{id}");
        self.f().frames.push(Frame::Exc(lbl.clone()));
        let bb = self.block(body)?;
        self.f().frames.pop();
        let mut b = format!("{{ let __r{id}: Result<(), E> = {lbl}: {{ {bb}\nOk(()) }};\nmatch __r{id} {{ Ok(()) => {{ ");
        if let Some(e) = els {
            b.push_str(&self.block(e)?);
        }
        write!(b, "}} Err(E(__e{id})) => {{ ").unwrap();
        let ev = format!("__e{id}");
        let mut first = true;
        let mut caught_all = false;
        for h in handlers {
            let test = match &h.types {
                None => {
                    caught_all = true;
                    "true".to_string()
                }
                Some(t) => {
                    let tt = self.ex(t, line)?;
                    self.q(&format!("exc_match(&{ev}, &{tt})"))
                }
            };
            if !first {
                b.push_str(" else ");
            }
            first = false;
            self.f().exc_vars.push(ev.clone());
            let bind = match &h.name {
                Some(n) => self.store(n, &format!("{ev}.clone()")),
                None => String::new(),
            };
            // Python deletes the `as` name when the handler ends, also when it
            // ends by break, continue or return (CPython: `finally: e = None; del e`).
            let hb = match &h.name {
                Some(n) if has_jump(&h.body) && !has_yield(&h.body) => {
                    let nm = Ex::Name(n.clone());
                    let fin = vec![Stmt { line: h.line, st: St::Assign(vec![nm.clone()], Ex::NoneV) }, Stmt { line: h.line, st: St::Del(vec![nm]) }];
                    let wrapped = [Stmt { line: h.line, st: St::Try(h.body.clone(), vec![], None, Some(fin)) }];
                    self.block(&wrapped)?
                }
                _ => self.block(&h.body)?,
            };
            self.f().exc_vars.pop();
            let unbind = match &h.name {
                Some(n) => self.store(n, "V::Unbound"),
                None => String::new(),
            };
            let guard = if has_yield(&h.body) { String::new() } else { format!("let _exg{id} = ExcGuard::push({ev}.clone());") };
            write!(b, "if {test} {{ {guard}{bind}\n{hb}\n{unbind} }}").unwrap();
        }
        if !caught_all {
            let r = self.raise_code(&format!("E({ev})"));
            write!(b, " else {{ {r}; }}").unwrap();
        }
        b.push_str(" } } }");
        Ok(b)
    }
    fn flow_dispatch(&mut self, id: usize, line: usize) -> ER<String> {
        let ret = if self.fstack.last().unwrap().is_module { "Flow::Ret(_) => {}".to_string() } else { format!("Flow::Ret(__v) => {{ {} }}", self.jump("return", "__v", line)?) };
        let brk = match self.loop_or_fin() {
            true => format!("Flow::Brk => {{ {} }} Flow::Cont => {{ {} }}", self.jump("break", "", line)?, self.jump("continue", "", line)?),
            false => "Flow::Brk | Flow::Cont => {}".to_string(),
        };
        let exc = self.raise_code("__e");
        Ok(format!("match __fl{id} {{ Flow::Normal => {{}} {ret} {brk} Flow::Exc(__e) => {{ {exc}; }} }}"))
    }
    fn loop_or_fin(&self) -> bool {
        self.fstack.last().unwrap().frames.iter().any(|f| matches!(f, Frame::Loop(_)))
    }
    fn with_stmt(&mut self, items: &[(Ex, Option<Ex>)], body: &[Stmt], line: usize) -> ER<String> {
        if items.len() > 1 {
            let inner = Stmt { line, st: St::With(items[1..].to_vec(), body.to_vec()) };
            return self.with_stmt(&items[..1], &[inner], line);
        }
        let (e, t) = &items[0];
        if let Ex::Call(f, _) = e {
            if matches!(&**f, Ex::Name(n) if n == "open") && !self.bridge.on && !self.user_modules.iter().any(|m| m == "_brevi_open") {
                return Err(err(line, "open() is not supported in dynamic builds"));
            }
        }
        let id = self.id();
        let ee = self.ex(e, line)?;
        let enter = self.q(&format!("call_method(&__m{id}, \"__enter__\", Args::p(vec![]))"));
        let bind = match t {
            Some(t) => self.assign(t, &format!("__en{id}"), line)?,
            None => String::new(),
        };
        let lbl = format!("'f{id}");
        self.f().frames.push(Frame::Fin(lbl.clone()));
        let bb = self.block(body)?;
        self.f().frames.pop();
        // __exit__ runs at the with's line
        let w = self.wsite(line, true);
        let exit_ok = self.q(&format!("{{ {w} call_method(&__m{id}, \"__exit__\", Args::p(vec![V::None, V::None, V::None])) }}"));
        let exit_err = self.q(&format!("{{ {w} call_method(&__m{id}, \"__exit__\", Args::p(vec![V::Class(type_of(&__ev)?), __ev.clone(), V::None])) }}"));
        let sup = self.q("truth(&__sx)");
        let after = self.flow_dispatch(id, line)?;
        Ok(format!(
            "{{ let __m{id} = {ee}; let __en{id} = {enter}; {bind} let mut __fl{id}: Flow = {lbl}: {{ {bb}\nFlow::Normal }};\n if let Flow::Exc(E(__ev)) = &__fl{id} {{ let __ev = __ev.clone(); let __sx = {exit_err}; if {sup} {{ __fl{id} = Flow::Normal; }} }} else {{ {exit_ok}; }}\n{after} }}"
        ))
    }

    // ---- functions and classes ----
    fn def(&mut self, d: &FuncDef) -> ER<String> {
        if has_yield(&d.body) && !d.is_async {
            if !GEN_OK {
                return Err(err(d.line, "generators (yield) are not supported in dynamic builds on this platform"));
            }

        }
        if d.is_async && !GEN_OK {
            return Err(err(d.line, "async functions are not supported in dynamic builds on this platform"));
        }
        self.next_async = d.is_async;
        // `@types.coroutine` innermost on a plain generator function: CPython sets CO_ITERABLE_COROUTINE on the
        // def's code and returns the function itself, so the def is built with that flag and the decorator is not
        // called. Any other use of types.coroutine stays refused (lib_attr_check).
        let iter_coro = d.decorators.last().is_some_and(|x| self.is_types_coroutine(x));
        if iter_coro && (d.is_async || !has_yield(&d.body)) {
            return Err(err(d.line, "types.coroutine on a function that is not a generator function is not supported in dynamic builds"));
        }
        self.next_iter_coro = iter_coro;
        let own = d.disp.as_deref().unwrap_or(&d.name);
        let qn = self.qualname_of(own);
        let v = self.closure(own, &qn, &d.params, &d.body, d.line, true);
        self.next_async = false;
        self.next_iter_coro = false;
        let mut v = v?;
        v = format!("with_qualname({v}, {})", lit(&qn));
        if let Some(doc) = docstring(&d.body) {
            v = format!("with_doc({v}, {})", lit(&doc));
        }
        if self.module != "__main__" {
            v = format!("with_module({v}, {})", lit(&self.module));
        }
        // the def's annotations, evaluated now (after the defaults, as CPython does), while the program reads
        // __annotations__ somewhere: parameters in order, then "return"
        let anns = super::typing::ANN_DEFS.load(std::sync::atomic::Ordering::Relaxed);
        if !anns && self.eval_ann && (!d.params.ann.is_empty() || d.ret.is_some()) {
            for a in d.params.ann.iter().map(|(_, a)| a).chain(d.ret.iter()) {
                self.pyd_ann(a, d.line, false)?;
            }
            // evaluated now and dropped, as CPython 3.13 evaluates them when the def runs; a PEP 695 parameter
            // is the TypeVar CPython's annotation scope binds
            self.ann_tvars = d.tparams.clone();
            let mut evs = String::new();
            let mut res = Ok(());
            for a in d.params.ann.iter().map(|(_, a)| a).chain(d.ret.iter()) {
                match self.ex(a, d.line) {
                    Ok(x) => evs.push_str(&format!("let _ = {x}; ")),
                    Err(e) => {
                        res = Err(e);
                        break;
                    }
                }
            }
            self.ann_tvars.clear();
            res?;
            v = format!("{{ let __fv = {v}; {evs}__fv }}");
        }
        if anns {
            let mut kv = vec![];
            for (pn, a) in d.params.ann.iter() {
                kv.push(format!("(s({}), {})", lit(pn), self.ex(a, d.line)?));
            }
            if let Some(r) = &d.ret {
                kv.push(format!("(s(\"return\"), {})", self.ex(r, d.line)?));
            }
            v = self.q(&format!("with_ann({v}, vec![{}])", kv.join(", ")));
        }
        // PEP 695 `def f[T]`: the type parameters, set before the decorators run (as CPython's annotation scope
        // does). The names themselves are not bound, so a def that uses one is refused.
        if !d.tparams.is_empty() {
            // (the def's own evaluated annotations bind the parameters; a nested def's or class's do not here)
            if super::typing::tparams_used(d, &d.tparams, anns) || (self.eval_ann && super::typing::tparams_in_nested_anns(&d.body, &d.tparams)) {
                return Err(err(d.line, "a PEP 695 type parameter used inside its def (`def f[T]` with T in a default, the body or an evaluated annotation) is not supported in dynamic builds"));
            }
            let tv: Vec<String> = d.tparams.iter().map(|n| format!("pep695_typevar({})", lit(n))).collect();
            v = format!("with_tparams({v}, tup(vec![{}]))", tv.join(", "));
        }
        if PYD_ON.load(std::sync::atomic::Ordering::Relaxed) {
            let n = if d.params.star.is_some() { 99 } else { d.params.pos.len() };
            v = format!("with_np({v}, {n})");
        }
        // `@some_dispatcher.register` (no explicit class) registers on the def's first annotation.
        if d.decorators.iter().any(|x| matches!(x, Ex::Attr(_, n) if n == "register")) {
            if let Some((pn, ann)) = d.params.ann.first() {
                let desc = self.sd_ann_desc(ann, d.line)?;
                v = format!("with_sd_ann({v}, tup(vec![s({}), {desc}]))", lit(pn));
            }
        }
        let n_dec = d.decorators.len() - usize::from(iter_coro);
        for dec in d.decorators[..n_dec].iter().rev() {
            let dd = self.ex(dec, d.line)?;
            v = self.q(&format!("call(&{dd}, Args::p(vec![{v}]))"));
        }
        Ok(v)
    }
    // `types.coroutine`, where `types` is certainly the types module (bound once, by `import types`)
    fn is_types_coroutine(&self, e: &Ex) -> bool {
        let Ex::Attr(o, n) = e else { return false };
        if n != "coroutine" {
            return false;
        }
        if self.static_module(o).as_deref() == Some("types") {
            return true;
        }
        matches!(&**o, Ex::Name(x) if matches!(self.lookup(x), Some(Var::Global(_))) && self.types_once.contains(&(self.module.clone(), x.clone())))
    }
    // A function value. Module-level functions (and methods of module-level classes) become plain Rust
    // functions plus a wrapper; nested ones are closures capturing the cells they use.
    fn closure(&mut self, name: &str, qualname: &str, p: &Params, body: &[Stmt], line: usize, is_def: bool) -> ER<String> {
        if !super::typing::SIG_DEFS.load(std::sync::atomic::Ordering::Relaxed) {
            return self.closure_body(name, qualname, p, body, line, is_def, None);
        }
        // the defaults are evaluated once, here, and both the function and its recorded signature hold them
        let id = self.id();
        let mut pre = String::new();
        let mut pd = vec![];
        let mut kd = vec![];
        for (k, (_, dv)) in p.pos.iter().enumerate() {
            pd.push(match dv {
                Some(x) => {
                    let c = self.ex(x, line)?;
                    write!(pre, "let __sdp{id}_{k} = {c}; ").unwrap();
                    format!("__sdp{id}_{k}.clone()")
                }
                None => "V::Unbound".into(),
            });
        }
        for (k, (_, dv)) in p.kwonly.iter().enumerate() {
            kd.push(match dv {
                Some(x) => {
                    let c = self.ex(x, line)?;
                    write!(pre, "let __sdk{id}_{k} = {c}; ").unwrap();
                    format!("__sdk{id}_{k}.clone()")
                }
                None => "V::Unbound".into(),
            });
        }
        let v = self.closure_body(name, qualname, p, body, line, is_def, Some((pd.clone(), kd.clone())))?;
        let names = |v: &[(String, Option<Ex>)]| v.iter().map(|(n, _)| lit(n)).collect::<Vec<_>>().join(", ");
        let opt = |n: &Option<String>| n.as_ref().map_or("None".to_string(), |n| format!("Some({})", lit(n)));
        Ok(format!(
            "{{ {pre}with_sig({v}, &[{}], {}, {}, &[{}], {}, vec![{}], vec![{}]) }}",
            names(&p.pos), p.posonly, opt(&p.star), names(&p.kwonly), opt(&p.kwstar), pd.join(", "), kd.join(", ")
        ))
    }
    #[allow(clippy::too_many_arguments)]
    fn closure_body(&mut self, name: &str, qualname: &str, p: &Params, body: &[Stmt], line: usize, is_def: bool, dflt: Option<(Vec<String>, Vec<String>)>) -> ER<String> {
        let id = self.id();
        let is_async = std::mem::take(&mut self.next_async);
        let is_gen = has_yield(body);
        // (a lambda in the def's defaults is built first: is_def keeps the flag for the def itself)
        let iter_coro = is_def && std::mem::take(&mut self.next_iter_coro) && is_gen && !is_async;
        let mk_gen = if iter_coro { "make_gen_ic" } else { "make_gen" };
        let params = lambda_params(p);
        let sc = scope_of(body, &params);
        let inner_free = nested_free(body);
        let at_top = self.fstack.len() == 1;
        let in_class_top = self.class_ctx.is_some() && self.fstack.len() == 2 && self.fstack[0].is_module;
        // defaults, evaluated now (or already, by closure, when the def records its signature)
        let (dflt_codes, kwd) = match dflt {
            Some(d) => d,
            None => {
                let mut dflt_codes = vec![];
                for (_, dv) in p.pos.iter() {
                    dflt_codes.push(match dv {
                        Some(x) => self.ex(x, line)?,
                        None => "V::Unbound".into(),
                    });
                }
                let mut kwd = vec![];
                for (_, dv) in p.kwonly.iter() {
                    kwd.push(match dv {
                        Some(x) => self.ex(x, line)?,
                        None => "V::Unbound".into(),
                    });
                }
                (dflt_codes, kwd)
            }
        };
        // signature: positional params, then kw-only params share the names list; bind() treats
        // kw-only as named params past npos.
        let mut names: Vec<String> = p.pos.iter().map(|x| x.0.clone()).collect();
        names.extend(p.kwonly.iter().map(|x| x.0.clone()));
        let npos = p.pos.len();
        let npo = p.posonly;
        let star = p.star.is_some();
        let kwstar = p.kwstar.is_some();
        let names_lit = names.iter().map(|n| lit(n)).collect::<Vec<_>>().join(", ");
        // captured cells from enclosing functions
        let mut captures: Vec<(String, String)> = vec![];
        let mut free = free_of_def(&params, body);
        if let Some(c) = &self.class_ctx {
            free.insert(c.clone());
        }
        for n in &free {
            if let Some(Var::Cell(r)) = self.lookup(n) {
                captures.push((n.clone(), r.clone()));
            }
        }
        // this function's frame
        let parent = self.fstack.last().unwrap();
        let (class_expr, self_name) = if self.class_ctx.is_some() { (self.class_ctx.clone(), p.pos.first().map(|x| x.0.clone())) } else { (parent.class_expr.clone(), parent.self_name.clone()) };
        let mut fr = Func { vars: HashMap::new(), frames: vec![], class_expr, self_name, qname: qualname.to_string(), exc_vars: vec![], is_module: false, is_class: false, enum_cnv: None, is_async, is_agen: is_async && is_gen, is_gen, direct_defs: HashMap::new(), cells: format!("k{id}_"), order: vec![], ns: None };
        for (n, r) in &captures {
            fr.vars.insert(n.clone(), Var::Cell(r.clone()));
        }
        {
            let text = format!("{body:?}");
            let at = |n: &String| text.find(&format!("{n:?}")).unwrap_or(usize::MAX);
            let mut local: Vec<&String> = sc.locals.iter().filter(|n| !params.contains(n) && !inner_free.contains(*n)).collect();
            local.sort_by_key(|n| (at(n), (*n).clone()));
            let mut cellv: Vec<&String> = sc.locals.iter().filter(|n| !params.contains(n) && inner_free.contains(*n)).collect();
            cellv.sort();
            let mut freev: Vec<&String> = captures.iter().map(|(n, _)| n).filter(|n| !sc.locals.contains(*n)).collect();
            freev.sort();
            fr.order = params.iter().cloned().chain(local.into_iter().cloned()).chain(cellv.into_iter().cloned()).chain(freev.into_iter().cloned()).collect();
        }
        let mut decls = String::new();
        let mut cells_init = String::new();
        let all_params: Vec<String> = params.clone();
        for n in sc.locals.iter() {
            let r = format!("v_{}", rust_ident(n));
            if inner_free.contains(n) {
                let c = format!("k{id}_{}", rust_ident(n));
                if all_params.contains(n) {
                    write!(cells_init, "let {c} = Rc::new(RefCell::new({r}.clone())); ").unwrap();
                } else if !is_gen && !is_async && self_recursive_def(body, n) {
                    // a nested function that calls itself holds its own cell: CellGuard breaks that loop on return
                    write!(decls, "let {c} = cell(); let __cg_{c} = CellGuard::new(&{c}); ").unwrap();
                    self.guards.insert(c.clone());
                } else {
                    write!(decls, "let {c} = cell(); ").unwrap();
                }
                fr.vars.insert(n.clone(), Var::Cell(c));
            } else {
                if !all_params.contains(n) {
                    write!(decls, "let mut {r}: V = V::Unbound; ").unwrap();
                }
                fr.vars.insert(n.clone(), Var::Local(r));
            }
        }
        for n in &sc.nonlocals {
            if let Some(Var::Cell(r)) = self.lookup(n) {
                fr.vars.insert(n.clone(), Var::Cell(r));
            } else {
                return Err(err(line, &format!("nonlocal '{n}' is not supported here")));
            }
        }
        for n in &sc.globals {
            let m = self.module.clone();
            let i = self.gslot(&m, n);
            fr.vars.insert(n.clone(), Var::Global(i));
        }
        let saved_ctx = self.class_ctx.take();
        self.fstack.push(fr);
        // nested defs that calls can reach directly (their captures are known now, with this frame on the stack)
        if !is_gen && !is_async {
            let depth = self.fstack.len();
            for n in sc.locals.iter() {
                if inner_free.contains(n) && !nonlocal_in(body, n) {
                    self.safe_cells.insert(format!("k{id}_{}", rust_ident(n)), depth);
                }
                if all_params.contains(n) {
                    continue;
                }
                if let Some(d) = direct_nested_def(body, n) {
                    let mut caps = vec![];
                    for x in &free_of_def(&lambda_params(&d.params), &d.body) {
                        if let Some(Var::Cell(r)) = self.lookup(x) {
                            caps.push(r);
                        }
                    }
                    // by the def's cell when functions nested here use it, else by its name in this frame
                    if inner_free.contains(n) {
                        let c = format!("k{id}_{}", rust_ident(n));
                        self.nested_direct.insert(c.clone(), (format!("dn_{c}"), caps, d.params.pos.len()));
                    } else {
                        let f = format!("dn{id}_{}", rust_ident(n));
                        self.fstack.last_mut().unwrap().direct_defs.insert(n.clone(), (f, caps, d.params.pos.len()));
                    }
                }
            }
        }
        let body_code = self.block(body);
        self.fstack.pop();
        self.class_ctx = saved_ctx;
        let body_code = body_code?;
        let pdecl: Vec<String> = params.iter().map(|n| format!("mut v_{}: V", rust_ident(n))).collect();
        let dkey = (self.module.clone(), name.to_string());
        let pre = if is_def && at_top { self.direct.get(&dkey).cloned() } else { None };
        let fname = match &pre {
            Some(p) => p.0.clone(),
            None => format!("d{id}_{}", rust_ident(name)),
        };
        // the def's plain __name__ and CPython's dotted __qualname__ (Sig.name is the qualname, so argument
        // errors say `K.m() ...` / `outer.<locals>.need() ...` as CPython does)
        let name_lit = lit(name);
        let qname = lit(qualname);
        let fq = lit(&if self.module == "builtins" { qualname.to_string() } else { format!("{}.{}", self.module, qualname) });
        // co_flags, as CPython sets them: CO_OPTIMIZED | CO_NEWLOCALS, plus the function's kind and nesting.
        let mut code_flags: u32 = 0x1 | 0x2;
        if star {
            code_flags |= 0x4; // CO_VARARGS
        }
        if kwstar {
            code_flags |= 0x8; // CO_VARKEYWORDS
        }
        if is_async && is_gen {
            code_flags |= 0x200; // CO_ASYNC_GENERATOR
        } else {
            if is_gen {
                code_flags |= 0x20; // CO_GENERATOR
            }
            if iter_coro {
                code_flags |= 0x100; // CO_ITERABLE_COROUTINE
            }
            if is_async {
                code_flags |= 0x80; // CO_COROUTINE
            }
        }
        if !(at_top || in_class_top) {
            code_flags |= 0x10; // CO_NESTED
        }
        let code = code_flags.to_string();
        let sig = format!("Sig {{ name: {qname}, params: &[{names_lit}], npos: {npos}, npo: {npo}, star: {star}, kwstar: {kwstar} }}");
        let dflt_all: Vec<String> = dflt_codes.iter().chain(kwd.iter()).cloned().collect();
        let nparams = params.len();
        let unpack: String = (0..nparams).map(|k| format!("__b.next().unwrap()")).collect::<Vec<_>>().join(", ");
        if captures.is_empty() && (at_top || in_class_top) {
            // a plain Rust function
            if is_async {
                let mk = if is_gen { "make_agen" } else { "make_acoro" };
                write!(self.items, "fn {fname}({}) -> R {{ Ok({mk}(sym({qname}), move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}\n", pdecl.join(", ")).unwrap();
            } else if is_gen {
                write!(self.items, "fn {fname}({}) -> R {{ Ok({mk_gen}(sym({qname}), move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}\n", pdecl.join(", ")).unwrap();
            } else {
                write!(self.items, "fn {fname}({}) -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }}\n", pdecl.join(", ")).unwrap();
            }
            let need_defaults = dflt_all.iter().any(|d| d != "V::Unbound");
            if is_def && at_top && !star && !kwstar && p.kwonly.is_empty() {
                // direct calls: defaults kept in global slots
                let mut slots = vec![];
                let mut sets = String::new();
                let nd = p.pos.iter().rev().take_while(|x| x.1.is_some()).count();
                for k in (npos - nd)..npos {
                    let m = self.module.clone();
                    let slot = match &pre {
                        Some(p) => p.2[k - (npos - nd)],
                        None => self.gslot(&m, &format!("<default {id} {k}>")),
                    };
                    slots.push(slot);
                    write!(sets, "gs({slot}, {}); ", dflt_codes[k]).unwrap();
                }
                let dv: Vec<String> = (0..npos).map(|k| if k >= npos - nd { format!("gr({})", slots[k - (npos - nd)]) } else { "V::Unbound".into() }).collect();
                let v = format!("with_fq(with_code({{ {sets} func({name_lit}, move |a: Args| {{ let __d = [{}]; let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {fname}({unpack}) }}) }}, {code}), {fq})", dv.join(", "));
                // no free variables: __closure__ is None
                // (a method with a zero-argument super() has CPython's __class__ cell, which this path does not hold:
                // its __closure__ stays unrecorded, so reading it is refused rather than answered None)
                let v = if self.has_closure && !(self.class_ctx.is_some() && body_has_zero_super(body)) { format!("with_closure({v}, vec![])") } else { v };
                if !is_async && !is_gen && (1..=4).contains(&nparams) {
                    return Ok(format!("with_fast({v}, FastFn::F{nparams}({fname}))"));
                }
                return Ok(v);
            }
            let _ = need_defaults;
            let v = format!("with_fq(with_code({{ let __d = vec![{}]; func({name_lit}, move |a: Args| {{ let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {fname}({unpack}) }}) }}, {code}), {fq})", dflt_all.join(", "));
            // (a method with a zero-argument super() has CPython's __class__ cell, which this path does not hold:
                // its __closure__ stays unrecorded, so reading it is refused rather than answered None)
                let v = if self.has_closure && !(self.class_ctx.is_some() && body_has_zero_super(body)) { format!("with_closure({v}, vec![])") } else { v };
            // methods (and defs not called directly): calls passing every parameter by position skip bind()
            if !is_async && !is_gen && !star && !kwstar && p.kwonly.is_empty() && (1..=4).contains(&nparams) {
                return Ok(format!("with_fast({v}, FastFn::F{nparams}({fname}))"));
            }
            return Ok(v);
        }
        // a closure
        let caps: String = captures.iter().map(|(_, r)| format!("let {r} = {r}.clone(); ")).collect();
        // The cells __closure__ exposes: the same ones, minus a class cell the body never uses (CPython does
        // not expose that one, and the body's FnBox does not hold it either).
        let class_name = self.class_ctx.clone();
        let need_class = class_name.is_some() && body_has_zero_super(body);
        let closure_captures: Vec<&(String, String)> = captures
            .iter()
            .filter(|(n, _)| Some(n.as_str()) != class_name.as_deref() || need_class)
            .collect();
        // clones made before `caps` moves its own into the function
        let (wo, wc) = if self.has_closure {
            (format!("{{ let __clos = vec![{}]; with_closure(", closure_captures.iter().map(|(_, r)| format!("{r}.clone(), ")).collect::<String>()), ", __clos) }")
        } else {
            (String::new(), "")
        };
        let bind_params: String = params.iter().map(|n| format!("let mut v_{} = __b.next().unwrap(); ", rust_ident(n))).collect();
        // a nested def that calls reach directly: a plain Rust function taking its cells, and a wrapper
        if is_def && !is_gen && !is_async {
            let nd = match self.lookup(name) {
                Some(Var::Cell(c)) => self.nested_direct.get(&c).cloned(),
                Some(Var::Local(_)) => self.fstack.last().unwrap().direct_defs.get(name).cloned(),
                _ => None,
            };
            if let Some((dfn, dcaps, _)) = nd {
                if dcaps == captures.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>() {
                    let cparams: String = dcaps.iter().map(|r| format!("{r}: Rc<RefCell<V>>, ")).collect();
                    write!(self.items, "fn {dfn}({cparams}{}) -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }}\n", pdecl.join(", ")).unwrap();
                    let cargs: String = dcaps.iter().map(|r| format!("{r}.clone(), ")).collect();
                    return Ok(format!("with_fq({wo}with_code({{ let __d = vec![{}]; {caps} func({name_lit}, move |a: Args| -> R {{ let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {dfn}({cargs}{unpack}) }}) }}, {code}){wc}, {fq})", dflt_all.join(", ")));
                }
                return Err(err(line, "internal: a nested function's captures changed"));
            }
        }
        if is_gen || is_async {
            let mk = if is_async && is_gen { format!("make_agen(sym({qname}), ") } else if is_async { format!("make_acoro(sym({qname}), ") } else { format!("{mk_gen}(sym({qname}), ") };
            return Ok(format!(
                "with_fq({wo}with_code({{ let __d = vec![{}]; {caps} func({name_lit}, move |a: Args| -> R {{ let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {bind_params}{caps} Ok({mk}move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}) }}, {code}){wc}, {fq})",
                dflt_all.join(", ")
            ));
        }
        Ok(format!(
            "with_fq({wo}with_code({{ let __d = vec![{}]; {caps} func({name_lit}, move |a: Args| -> R {{ let __depth = Depth::new()?; let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {bind_params}{decls}{cells_init}\n{body_code}\nOk(V::None) }}) }}, {code}){wc}, {fq})",
            dflt_all.join(", ")
        ))
    }
    fn class(&mut self, c: &ClassDef) -> ER<String> {
        let id = self.id();
        let mut bases = vec![];
        // a subclass of memoryview, complex, staticmethod or classmethod has no wrapped value here (bytes,
        // bytearray, str, int and the containers do)
        for b in &c.bases {
            if let Ex::Name(n) = b {
                if matches!(n.as_str(), "memoryview" | "complex" | "staticmethod" | "classmethod") && self.lookup(n).is_none() {
                    return Err(err(c.line, &format!("a subclass of {n} is not supported in dynamic builds")));
                }
            }
        }
        for b in &c.bases {
            bases.push(self.ex(b, c.line)?);
        }
        // Bases are evaluated before the class body, as CPython does; the body needs them to know
        // whether it is defining an Enum (auto() turns into its value on assignment).
        let mut base_decls = String::new();
        let mut base_vars: Vec<String> = vec![];
        for (k, b) in bases.iter().enumerate() {
            let vn = format!("__base{id}_{k}");
            write!(base_decls, "let {vn}: V = {b}; ").unwrap();
            base_vars.push(vn);
        }
        // the class body runs with its own namespace: class-level names are Rust locals here,
        // invisible to the methods. When the body has an annotated statement, __annotations__ starts as
        // CPython's SETUP_ANNOTATIONS does: an empty dict, filled as the statements run. (Never under
        // `from __future__ import annotations`, where annotations are strings the parser does not keep;
        // and a class with no annotations has no __annotations__ of its own until one is read, which
        // getattr in rt_call lazily makes.)
        let mut sc = scope_of(&c.body, &[]);
        let has_meta = c.kwargs.iter().any(|(k, _)| k == "metaclass");
        // a metaclass sees the namespace, so with one the class's annotations are always made, as CPython makes them
        let setup = !self.lazy && (self.keep_ann || has_meta) && c.body.iter().any(|s| matches!(s.st, St::AnnAssign(..)));
        if has_meta && c.body.iter().any(|s| matches!(s.st, St::AnnAssign(..))) {
            fn plain(e: &Ex) -> bool {
                match e {
                    Ex::Name(_) | Ex::Str(_) | Ex::NoneV => true,
                    Ex::Attr(o, _) => plain(o),
                    _ => false,
                }
            }
            if self.lazy || c.body.iter().any(|s| matches!(&s.st, St::AnnAssign(_, _, a, _) if !plain(a))) {
                return Err(err(c.line, "a class with a metaclass annotated with a type expression (such as list[int]), or under `from __future__ import annotations`, is not supported in dynamic builds"));
            }
        }
        if setup {
            sc.locals.insert("__annotations__".into());
        }
        let own_name = c.disp.clone().unwrap_or_else(|| c.name.clone());
        let sattrs = c.static_attrs.iter().map(|n| lit(n)).collect::<Vec<_>>().join(", ");
        // A class body that calls locals() must run with its namespace as a dict: locals() returns that
        // dict, and every name load and store goes through it (CPython's LOAD_FROM_DICT_OR_DEREF / LOAD_NAME
        // and DELETE_NAME). Other classes keep the plain Rust locals exactly as before.
        // a metaclass (`class M(type)`, or built on another class of this module that is one) may define only the
        // special methods the runtime asks it for (META_DUNDERS in rt_call.rs, which also checks at run time)
        if c.bases.iter().any(|b| matches!(b, Ex::Name(n) if (n == "type" && self.lookup("type").is_none()) || self.metaclasses.contains(n))) {
            self.metaclasses.insert(c.name.clone());
            for s in &c.body {
                let names: Vec<&String> = match &s.st {
                    St::Def(d) => vec![&d.name],
                    St::Assign(ts, _) => ts.iter().filter_map(|t| if let Ex::Name(n) = t { Some(n) } else { None }).collect(),
                    St::AnnAssign(Ex::Name(n), v, _, simple) if *simple || v.is_some() => vec![n],
                    _ => vec![],
                };
                if let Some(n) = names.into_iter().find(|n| n.len() > 4 && n.starts_with("__") && n.ends_with("__") && !META_DUNDERS.contains(&n.as_str())) {
                    return Err(err(s.line, &format!("a metaclass defining {n} is not supported in dynamic builds")));
                }
            }
        }
        // `metaclass=M`: the body runs in M.__prepare__'s namespace dict, and M(name, bases, ns) makes the class
        let meta = c.kwargs.iter().find(|(k, _)| k == "metaclass").map(|(_, e)| e);
        if meta.is_some() && c.body.iter().any(|s| {
            let mut hit = false;
            crate::typed_guard::walk_st(&s.st, &mut |e| hit |= matches!(e, Ex::Name(n) if n == "__annotations__"));
            hit
        }) {
            return Err(err(c.line, "a class with a metaclass whose body names __annotations__ is not supported in dynamic builds"));
        }
        let dict_mode = meta.is_some() || class_calls_locals(&c.body);
        let parent = self.fstack.last().unwrap();
        // CPython's __qualname__: `Outer.Inner` inside a class, `f.<locals>.C` inside a function.
        let qualname = self.qualname_of(&own_name);
        // __firstlineno__ is the class statement's line; in Brevi's own copy of a CPython module, the line of
        // the class in CPython's file. Why the dict cannot be CPython's, when it cannot ("" when it can):
        let mut first = c.first;
        let mut why = "";
        if IN_COPY.with(|k| k.get()) {
            match super::cpy_lines::CPY_CLASS_LINES.iter().find(|(m, q, _)| *m == self.module && *q == qualname) {
                Some((_, _, l)) => first = *l,
                None => why = "a class of Brevi's own copy of a module",
            }
        }
        // a class whose annotations Brevi does not keep lacks the __annotations__ CPython's dict has
        if !(setup || !c.body.iter().any(|s| matches!(s.st, St::AnnAssign(..)))) {
            why = "annotations Brevi did not keep";
        }
        let ns_var = format!("__ns{id}");
        let mut fr = Func { vars: HashMap::new(), frames: parent.frames.clone(), class_expr: parent.class_expr.clone(), self_name: parent.self_name.clone(), qname: qualname.clone(), exc_vars: parent.exc_vars.clone(), is_module: false, is_class: true, enum_cnv: None, is_async: false, is_agen: false, is_gen: false, direct_defs: HashMap::new(), cells: String::new(), order: vec![], ns: dict_mode.then_some(ns_var.clone()) };
        let ctx = format!("__auto_ctx{id}");
        let av = format!("__auto_last{id}");
        let mut decls = String::new();
        write!(decls, "{base_decls}").unwrap();
        write!(decls, "let mut {ctx}: EnumCtx = enum_auto_ctx({}, &[{}]); let mut {av}: Vec<V> = vec![]; ", lit(&c.name), base_vars.iter().map(|v| format!("&{v}")).collect::<Vec<_>>().join(", ")).unwrap();
        if let Some(mx) = meta {
            // as __build_class__: the metaclass after the bases, then its __prepare__, then the body's own first
            // stores (__module__, __qualname__, __firstlineno__, SETUP_ANNOTATIONS, the docstring)
            let mv = self.ex(mx, c.line)?;
            write!(decls, "let __meta{id}: V = {mv}; ").unwrap();
            let prep = self.q(&format!("meta_prepare(&__meta{id}, {}, &tup(vec![{}]))", lit(&c.name), base_vars.iter().map(|v| format!("{v}.clone()")).collect::<Vec<_>>().join(", ")));
            write!(decls, "let mut {ns_var}: V = {prep}; ").unwrap();
            for (k, v) in [("__module__", format!("s({})", lit(if self.module == "__main__" { "__main__" } else { &self.module }))), ("__qualname__", format!("s({})", lit(&qualname))), ("__firstlineno__", format!("V::Int({first})"))] {
                let set = self.q(&format!("setitem(&{ns_var}, &s(\"{k}\"), {v})"));
                write!(decls, "{set}; ").unwrap();
            }
            if setup {
                let r = format!("a{id}___annotations__");
                let a = self.q(&format!("ns_annotations(&{ns_var})"));
                write!(decls, "let {r}: V = {a}; ").unwrap();
            }
            if let Some(d) = docstring(&c.body) {
                let set = self.q(&format!("setitem(&{ns_var}, &s(\"__doc__\"), s({}))", lit(&d)));
                write!(decls, "{set}; ").unwrap();
            }
            for n in sc.locals.iter() {
                fr.vars.insert(n.clone(), Var::NS(ns_var.clone()));
            }
        } else if dict_mode {
            write!(decls, "let mut {ns_var}: V = dict_new(); ").unwrap();
            // the namespace starts as CPython's class body starts it: __module__, __qualname__, __firstlineno__
            let modname = if self.module == "__main__" { "__main__".to_string() } else { self.module.clone() };
            for (k, v) in [("__module__", format!("s({})", lit(&modname))), ("__qualname__", format!("s({})", lit(&qualname))), ("__firstlineno__", format!("V::Int({first})"))] {
                let set = self.q(&format!("setitem(&{ns_var}, &s({}), {v})", lit(k)));
                write!(decls, "{set}; ").unwrap();
            }
            if setup {
                // SETUP_ANNOTATIONS runs first, so __annotations__ is the namespace dict's first key; the
                // alias here is the same dict the class body stores annotations into.
                let r = format!("a{id}___annotations__");
                let set = self.q(&format!("setitem(&{ns_var}, &s(\"__annotations__\"), {r}.clone())"));
                write!(decls, "let {r}: V = dict_new(); {set}; ").unwrap();
            }
            for n in sc.locals.iter() {
                fr.vars.insert(n.clone(), Var::NS(ns_var.clone()));
            }
        } else {
            for n in sc.locals.iter() {
                let r = format!("a{id}_{}", rust_ident(n));
                let init = if setup && n == "__annotations__" { "dict_new()" } else { "V::Unbound" };
                write!(decls, "let mut {r}: V = {init}; ").unwrap();
                fr.vars.insert(n.clone(), Var::Local(r));
            }
        }
        fr.enum_cnv = Some((av, ctx.clone()));
        let mut fields = vec![];
        for s in &c.body {
            if let St::AnnAssign(Ex::Name(n), _, ann, true) = &s.st {
                // dataclasses: ClassVar, InitVar and KW_ONLY are marked
                match ann_kind(ann) {
                    Some(k) => fields.push(lit(&format!("{k}:{n}"))),
                    None => fields.push(lit(n)),
                }
            }
        }
        let saved = self.class_ctx.replace(c.name.clone());
        self.fstack.push(fr);
        let mut body = String::new();
        let mut res = Ok(());
        for s in &c.body {
            let mut code = self.stmt_site(s);
            if let (true, St::AnnAssign(t, v, ann, simple)) = (setup, &s.st) {
                let line = s.line;
                // CPython (compiler_annassign) stores the value first, or with no value evaluates the target's
                // object; then it evaluates the annotation, and stores a simple name's in the class's
                // __annotations__ (setup implies annotations are not lazy).
                let first = match v {
                    Some(v) => self.ex(v, line).and_then(|vv| Ok(format!("{{ let __val = {vv}; {} }} ", self.assign(t, "__val", line)?))),
                    None => self.ann_target_only(t, line),
                };
                let av = first.and_then(|f| Ok((f, self.ex(ann, line)?)));
                let (f, av) = match av {
                    Ok(x) => x,
                    Err(e) => {
                        res = Err(e);
                        break;
                    }
                };
                code.push_str(&f);
                match t {
                    Ex::Name(n) if *simple => {
                        let r = format!("a{id}_{}", rust_ident("__annotations__"));
                        let set = self.q(&format!("setitem(&{r}, &__k, __av)"));
                        code.push_str(&format!("let __av = {av}; let __k = s({}); {set}; ", lit(n)));
                    }
                    _ => code.push_str(&format!("let _ = {av}; ")),
                }
            } else {
                match self.stmt(s) {
                    Ok(x) => code.push_str(&x),
                    Err(e) => {
                        res = Err(e);
                        break;
                    }
                }
            }
            body.push_str(&code);
            body.push('\n');
        }
        self.fstack.pop();
        self.class_ctx = saved;
        res?;
        // the class is made at the class statement's line
        let w = self.wsite(c.line, c.decorators.is_empty());
        let abcm = if c.abc_meta { format!("let __c = {};", self.q("abc_meta(__c)")) } else { String::new() };
        let mut v = if dict_mode {
            // the namespace dict holds the class attributes in the order the body bound them; the doc is
            // added only now, as __build_class__ adds it after the body
            if meta.is_some() {
                let sa: Vec<String> = c.static_attrs.iter().map(|n| format!("s({})", lit(n))).collect();
                let set = self.q(&format!("setitem(&{ns_var}, &s(\"__static_attributes__\"), tup(vec![{}]))", sa.join(", ")));
                let ig = self.q(&format!("enum_ignore_ns(&{ctx}, &{ns_var}, true)"));
                let set = format!("{set}; {ig}");
                let mk = self.q(&format!("meta_build(&__meta{id}, {}, {}, {}, vec![{}], {ns_var}.clone(), vec![{}])", lit(&own_name), lit(&qualname), lit(if self.module == "__main__" { "__main__" } else { &self.module }), base_vars.join(", "), fields.join(", ")));
                format!("{{ {decls}\n{body}\n{set}; {w} let __c = {mk}; __c }}")
            } else {
            let attrs = String::new();
            let items = self.q(&format!("dict_sym_items(&{ns_var})"));
            let mk = self.q(&format!("make_class_s({}, {}, {}, vec![{}], {items}, vec![{}], {}, &[{}], {})", lit(&own_name), lit(&qualname), lit(if self.module == "__main__" { "__main__" } else { &self.module }), base_vars.join(", "), fields.join(", "), first, sattrs, lit(why)));
            // a docstring is the body's first store (CPython's STORE_NAME __doc__), after SETUP_ANNOTATIONS
            let doc_set = match docstring(&c.body) {
                Some(d) => {
                    let set = self.q(&format!("setitem(&{ns_var}, &s(\"__doc__\"), s({}))", lit(&d)));
                    format!("{set}; ")
                }
                None => String::new(),
            };
            let ig = self.q(&format!("enum_ignore_ns(&{ctx}, &{ns_var}, false)"));
            format!("{{ {decls}\n{doc_set}{body}\n{attrs} {ig}; {w} let __c = {mk}; {abcm} __c }}")
            }
        } else {
            // class attributes in the order the body first binds them, as CPython's class namespace keeps them
            // (Enum members and vars() follow it)
            let mut order: Vec<&String> = vec![];
            for s in &c.body {
                let names: Vec<&String> = match &s.st {
                    St::Assign(ts, _) => ts.iter().filter_map(|t| if let Ex::Name(n) = t { Some(n) } else { None }).collect(),
                    St::AnnAssign(Ex::Name(n), v, _, simple) if *simple || v.is_some() => vec![n],
                    St::Def(d) => vec![&d.name],
                    St::Class(k) => vec![&k.name],
                    _ => vec![],
                };
                for n in names {
                    if sc.locals.contains(n) && !order.contains(&n) {
                        order.push(n);
                    }
                }
            }
            for n in sc.locals.iter() {
                if !order.contains(&n) {
                    order.push(n);
                }
            }
            let mut attrs = String::new();
            // __annotations__ is the first name the class namespace binds (SETUP_ANNOTATIONS runs before the
            // body; a later assignment to it keeps the first position, as a dict key does in CPython).
            if setup {
                let r = format!("a{id}___annotations__");
                write!(attrs, "__at.push((\"__annotations__\", {r})); ").unwrap();
            }
            // a docstring is the body's first store (CPython's STORE_NAME __doc__), after SETUP_ANNOTATIONS; a
            // class without one gets __doc__ = None from make_class, after the names type_new adds
            let doc = docstring(&c.body);
            if let Some(d) = &doc {
                if sc.locals.contains("__doc__") {
                    let r = format!("a{id}___doc__");
                    write!(attrs, "__at.push((\"__doc__\", if matches!({r}, V::Unbound) {{ s({}) }} else {{ {r} }})); ", lit(d)).unwrap();
                } else {
                    write!(attrs, "__at.push((\"__doc__\", s({}))); ", lit(d)).unwrap();
                }
            }
            for n in order {
                if setup && n == "__annotations__" || doc.is_some() && n == "__doc__" {
                    continue;
                }
                let r = format!("a{id}_{}", rust_ident(n));
                write!(attrs, "if !matches!({r}, V::Unbound) {{ __at.push(({}, {r})); }} ", lit(n)).unwrap();
            }
            let mk = self.q(&format!("make_class_s({}, {}, {}, vec![{}], __at, vec![{}], {}, &[{}], {})", lit(&own_name), lit(&qualname), lit(if self.module == "__main__" { "__main__" } else { &self.module }), base_vars.join(", "), fields.join(", "), first, sattrs, lit(why)));
            format!("{{ {decls}\n{body}\nlet mut __at: Vec<(Sym, V)> = vec![]; {attrs} enum_ignore_drop(&{ctx}, &mut __at); {w} let __c = {mk}; {abcm} __c }}")
        };
        for dec in c.decorators.iter().rev() {
            let dd = self.ex(dec, c.line)?;
            v = self.q(&format!("call(&{dd}, Args::p(vec![{v}]))"));
        }
        Ok(v)
    }
}

// `from __future__ import annotations` (a __future__ import is always at the top of the module).
fn future_annotations(body: &[Stmt]) -> bool {
    body.iter().any(|s| matches!(&s.st, St::ImportFrom(m, v) if m == "__future__" && v.iter().any(|(n, _)| n == "annotations")))
}

// What a class-body annotation tells dataclasses: ClassVar, InitVar or KW_ONLY (as a name, or in a string).
fn ann_kind(a: &Ex) -> Option<&'static str> {
    let head = match a {
        Ex::Index(o, _) => o.as_ref(),
        a => a,
    };
    let name = match head {
        Ex::Name(n) => n.as_str(),
        Ex::Attr(_, n) => n.as_str(),
        Ex::Str(t) => {
            let t = t.trim();
            let t = t.strip_prefix("typing.").or_else(|| t.strip_prefix("dataclasses.")).unwrap_or(t);
            return ["ClassVar", "InitVar", "KW_ONLY"].into_iter().find(|k| t == *k || t.starts_with(&format!("{k}[")));
        }
        _ => return None,
    };
    ["ClassVar", "InitVar", "KW_ONLY"].into_iter().find(|k| name == *k)
}

// CPython's __package__ for a file of the program: None for the script, the package for a package's
// __init__.py, else the name up to its last dot ("" for a top-level module).
fn mod_package(name: &str, path: &str) -> Option<String> {
    if name == "__main__" {
        return None;
    }
    if path.ends_with("__init__.py") {
        return Some(name.to_string());
    }
    Some(name.rsplit_once('.').map(|(p, _)| p.to_string()).unwrap_or_default())
}
// The docstring of a def, class or module body.
// A def's or class's docstring as CPython 3.13 stores it: tabs expanded, the first line's leading spaces and the
// common indentation of the later lines removed (_PyCompile_CleanDoc).
fn docstring(body: &[Stmt]) -> Option<String> {
    match body.first().map(|s| &s.st) {
        Some(St::Expr(Ex::Str(d))) => Some(clean_doc(d)),
        _ => None,
    }
}
fn clean_doc(d: &str) -> String {
    // str.expandtabs(8)
    let mut t = String::with_capacity(d.len());
    let mut col = 0usize;
    for ch in d.chars() {
        match ch {
            '\t' => {
                let n = 8 - col % 8;
                t.extend(std::iter::repeat(' ').take(n));
                col += n;
            }
            '\n' | '\r' => {
                t.push(ch);
                col = 0;
            }
            c => {
                t.push(c);
                col += 1;
            }
        }
    }
    let lines: Vec<&str> = t.split('\n').collect();
    let margin = lines
        .iter()
        .skip(1)
        .filter_map(|l| {
            let n = l.len() - l.trim_start_matches(' ').len();
            (n < l.len()).then_some(n)
        })
        .min()
        .unwrap_or(0);
    let mut out = String::with_capacity(t.len());
    for (i, l) in lines.iter().enumerate() {
        if i == 0 {
            out.push_str(l.trim_start_matches(' '));
        } else {
            out.push('\n');
            let lead = l.len() - l.trim_start_matches(' ').len();
            out.push_str(&l[lead.min(margin)..]);
        }
    }
    out
}

fn builtin_fast(n: &str) -> Option<String> {
    Some(match n {
        "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "range" | "type" | "slice" => format!("bi_{n}"),
        "object" => "bi_object".into(),
        "exit" | "quit" => "bi_exit".into(),
        _ if is_exc_name(n) => return None,
        "NotImplemented" | "Ellipsis" | "__name__" | "super" => return None,
        _ => format!("bi_{n}"),
    })
}

pub fn rust_ident(n: &str) -> String {
    let mut s = String::new();
    for c in n.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            s.push(c);
        } else {
            write!(s, "u{:x}", c as u32).unwrap();
        }
    }
    s
}

// The Rust program for a set of modules (the first is __main__).
// An error names the module whose code it is in, so the message can name that module's file.
// inline caches for attribute access (getattr_ic): the program declares IC0..IC{n-1}
thread_local! {
    static IC_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    // the inline caches below this are the library copies' (emitted with them, see program)
    static IC_LIB_END: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn new_ic() -> usize {
    IC_COUNT.with(|c| {
        let k = c.get();
        c.set(k + 1);
        k
    })
}
pub fn ic_reset() {
    IC_COUNT.with(|c| c.set(0));
    IC_LIB_END.with(|c| c.set(0));
}
pub fn ic_statics() -> String {
    let n = IC_COUNT.with(|c| c.get());
    let mut s = String::new();
    for k in IC_LIB_END.with(|c| c.get())..n {
        s.push_str(&format!("static IC{k}: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);\n"));
    }
    s
}
// Whether `src` names a builtin iterator type (zip, map, filter, enumerate, reversed, or an itertools one) other
// than to call it or import it: `(repeat, args)`, `cls = chain`.
pub fn program(mods: &[Module], bridge: Bridge) -> Result<(String, Vec<String>), (String, usize, String)> {
    for m in mods {
        if let Some((line, n)) = super::typing::datetime_subclass(&m.body) {
            return Err((m.name.clone(), line, format!("a subclass of {n} is not supported in dynamic builds")));
        }
        if let Some((line, what)) = super::typing::enum_refusal(&m.body) {
            return Err((m.name.clone(), line, format!("{what} is not supported in dynamic builds")));
        }
    }
    // the program (not Brevi's library copies) uses the warnings module
    // (or functools.partial, whose __get__ warns at the line that reads it from an instance, or
    // collections.abc.ByteString, whose metaclass warns at the isinstance or class statement)
    let iter_pickle = mods.iter().any(|m| {
        let b = format!("{:?}", m.body);
        bridge.srcs.contains_key(&m.name)
            && b.contains("\"itertools\"")
            && ["\"pickle\"", "\"copy\"", "\"deepcopy\"", "\"__reduce__\"", "\"__reduce_ex__\"", "\"__setstate__\""].iter().any(|k| b.contains(k))
    });
    // (or pickles or copies itertools iterators, whose __reduce__ and __setstate__ warn at the caller's line)
    let track_warn = mods.iter().any(|m| {
        let b = format!("{:?}", m.body);
        bridge.srcs.contains_key(&m.name) && (b.contains("\"warnings\"") || b.contains("\"functools\"") && b.contains("\"partial\"") || b.contains("\"ByteString\""))
    }) || iter_pickle;

    let has_del = mods.iter().any(|m| format!("{:?}", m.body).contains("\"__del__\""));
    let has_closure = mods.iter().any(|m| format!("{:?}", m.body).contains("__closure__"));
    let mut em = Em {
        has_del,
        has_closure,
        guards: HashSet::new(),
        nested_direct: HashMap::new(),
        safe_cells: HashMap::new(),
        out: String::new(),
        items: String::new(),
        n: 0,
        globals: HashMap::new(),
        nglobals: 0,
        consts: vec![],
        const_idx: HashMap::new(),
        lone_idx: HashMap::new(),
        fstack: vec![],
        module: String::new(),
        user_modules: mods.iter().skip(1).map(|m| m.name.clone()).collect(),
        mod_info: mods.iter().filter_map(|m| m.path.as_ref().map(|p| (m.name.clone(), (p.clone(), docstring(&m.body), mod_package(&m.name, p))))).collect(),
        direct: HashMap::new(),
        mod_alias: HashMap::new(),
        types_alias: HashSet::new(),
        types_once: HashSet::new(),
        next_iter_coro: false,
        mod_names: HashMap::new(),
        class_ctx: None,
        lazy: false,
        keep_ann: false,
        eval_ann: false,
        ann_tvars: vec![],
        metaclasses: BTreeSet::new(),
        next_async: false,
        direct_ok: HashSet::new(),
        bridge,
        bridged_used: BTreeSet::new(),
        uses_re: false,
        re_names: HashSet::new(),
        track_warn,
        multi: HashMap::new(),
        wmods: HashSet::new(),
        lam_line: None,
        warn_names: HashSet::new(),
        set_lits: HashMap::new(),
    };
    // Brevi's own library copies first, each group named and numbered on its own, then the program's modules:
    // the copies' Rust (everything before LIB_END) is then the same for every program that imports them, so a
    // build can compile it once into a cached library (dynpy::compile) instead of with every program.
    let own = LIB_COPIES.lock().unwrap().clone();
    let mut libs: Vec<&Module> = mods.iter().skip(1).filter(|m| own.contains(&m.name)).collect();
    libs.sort_by(|a, b| a.name.cmp(&b.name));
    let users: Vec<&Module> = mods.iter().filter(|m| !libs.iter().any(|l| l.name == m.name)).collect();
    let mut lib_part = String::new();
    let mut inits = String::new();
    // Only the program's own modules decide (Brevi's copies never keep class annotations: their Rust must not
    // depend on the program), and never with Brevi's pydantic, which gives classes descriptor annotations (pyd.rs).
    // dataclasses reads a class's __annotations__ (field types, __doc__, classes made by type()), so importing it
    // keeps them too.
    let copies = OWN_COPIES.lock().unwrap().clone();
    let users_read_ann = !PYD_ON.load(std::sync::atomic::Ordering::Relaxed)
        && users.iter().any(|m| !copies.contains(&m.name) && (mentions_annotations(&m.body) || super::typing::imports(&m.body, "dataclasses")));
    for (gi, group) in [libs.clone(), users].into_iter().enumerate() {
        em.keep_ann = gi == 1 && users_read_ann;
    // module-level names, import aliases and directly callable defs
        for m in group.iter().copied() {
            let sc = scope_of(&m.body, &[]);
            let mut names = sc.locals.clone();
            // names declared global in functions
            fn globals_in(body: &[Stmt], out: &mut BTreeSet<String>) {
                for s in body {
                    match &s.st {
                        St::Def(d) => {
                            let sc = scope_of(&d.body, &[]);
                            out.extend(sc.globals.iter().cloned());
                            globals_in(&d.body, out);
                        }
                        St::Class(c) => globals_in(&c.body, out),
                        St::If(arms, els) => {
                            for (_, b) in arms {
                                globals_in(b, out);
                            }
                            if let Some(b) = els {
                                globals_in(b, out);
                            }
                        }
                        St::For(_, _, b, e) | St::While(_, b, e) => {
                            globals_in(b, out);
                            if let Some(e) = e {
                                globals_in(e, out);
                            }
                        }
                        St::Try(b, hs, e, f) => {
                            globals_in(b, out);
                            for h in hs {
                                globals_in(&h.body, out);
                            }
                            if let Some(e) = e {
                                globals_in(e, out);
                            }
                            if let Some(f) = f {
                                globals_in(f, out);
                            }
                        }
                        St::With(_, b) => globals_in(b, out),
                        _ => {}
                    }
                }
            }
            globals_in(&m.body, &mut names);
            // how often each name is bound at module level (top level statements only)
            let mut binds: HashMap<String, usize> = HashMap::new();
            let mut import_of: HashMap<String, String> = HashMap::new();
            let mut def_names: HashSet<String> = HashSet::new();
            fn count(body: &[Stmt], binds: &mut HashMap<String, usize>, import_of: &mut HashMap<String, String>, defs: &mut HashSet<String>, top: bool) {
                for s in body {
                    let mut b = BTreeSet::new();
                    match &s.st {
                        St::Import(v) => {
                            for (m, a) in v {
                                let bind = a.clone().unwrap_or_else(|| m.split('.').next().unwrap().to_string());
                                let target = if a.is_some() { m.clone() } else { m.split('.').next().unwrap().to_string() };
                                import_of.insert(bind.clone(), target);
                                b.insert(bind);
                            }
                        }
                        St::Def(d) => {
                            if top && d.decorators.is_empty() {
                                defs.insert(d.name.clone());
                            }
                            b.insert(d.name.clone());
                        }
                        _ => {
                            let sc = scope_of(std::slice::from_ref(s), &[]);
                            b = sc.locals;
                        }
                    }
                    for n in b {
                        *binds.entry(n).or_default() += 1;
                    }
                }
            }
            count(&m.body, &mut binds, &mut import_of, &mut def_names, true);
            // assignments to globals from inside functions also rebind
            let mut fn_globals = BTreeSet::new();
            globals_in(&m.body, &mut fn_globals);
            for (n, t) in import_of {
                if t == "types" {
                    em.types_alias.insert((m.name.clone(), n.clone()));
                    if binds.get(&n) == Some(&1) && !fn_globals.contains(&n) {
                        em.types_once.insert((m.name.clone(), n.clone()));
                    }
                }
                if binds.get(&n) == Some(&1) && !fn_globals.contains(&n) && !(em.bridge.has(&t) && !builtin_mod(t.as_str()) && !em.user_modules.contains(&t)) {
                    em.mod_alias.insert((m.name.clone(), n), t);
                }
            }
            for s in &m.body {
                if let St::Def(d) = &s.st {
                    let n = &d.name;
                    let p = &d.params;
                    if def_names.contains(n) && binds.get(n) == Some(&1) && !fn_globals.contains(n) && p.star.is_none() && p.kwstar.is_none() && p.kwonly.is_empty() && !has_yield(&d.body) && !d.is_async {
                        em.direct_ok.insert((m.name.clone(), n.clone()));
                        let npos = p.pos.len();
                        let nd = p.pos.iter().rev().take_while(|x| x.1.is_some()).count();
                        let slots: Vec<usize> = (npos - nd..npos).map(|k| em.gslot(&m.name, &format!("<default {n} {k}>"))).collect();
                        em.direct.insert((m.name.clone(), n.clone()), (format!("f_{}_{}", rust_ident(&m.name), rust_ident(n)), npos, slots));
                    }
                }
            }
            for n in &names {
                em.gslot(&m.name, n);
            }
            em.mod_names.insert(m.name.clone(), names);
        }
    // bodies
        for m in group.iter().copied() {
            em.module = m.name.clone();
            em.lazy = future_annotations(&m.body);
            IN_COPY.with(|c| c.set(OWN_COPIES.lock().unwrap().contains(&m.name)));
            em.eval_ann = !em.lazy && gi == 1 && !copies.contains(&m.name) && !BV_SPELLING.load(std::sync::atomic::Ordering::Relaxed);
            let mut fr = Func { vars: HashMap::new(), frames: vec![], class_expr: None, self_name: None, qname: String::new(), exc_vars: vec![], is_module: true, is_class: false, enum_cnv: None, is_async: false, is_agen: false, is_gen: false, direct_defs: HashMap::new(), cells: String::new(), order: vec![], ns: None };
            for n in em.mod_names[&m.name].clone() {
                let i = em.gslot(&m.name, &n);
                fr.vars.insert(n.clone(), Var::Global(i));
            }
            em.fstack.push(fr);
            let name_slot = em.gslot(&m.name, "__name__");
            let body = em.block(&m.body).map_err(|(l, e)| (m.name.clone(), l, e))?;
            em.fstack.pop();
            let ident = rust_ident(&m.name);
            let nm = lit(&m.name);
            // a copy's classes follow Brevi's copy, whose __class_getitem__s are not all CPython's (class_subscript)
            let mark = if IN_COPY.with(|c| c.get()) || gi == 0 { format!("copy_module({nm}); ") } else { String::new() };
            write!(inits, "thread_local! {{ static DONE_{ident}: Cell<bool> = Cell::new(false); }}\nfn init_{ident}() -> Result<(), E> {{ if DONE_{ident}.with(|d| d.replace(true)) {{ return Ok(()); }} {mark}gs({name_slot}, s({nm}));\n{body}\nOk(()) }}\n").unwrap();
        }
        if gi == 0 && !libs.is_empty() {
            lib_part.push_str(&std::mem::take(&mut em.items));
            lib_part.push_str(&std::mem::take(&mut inits));
            for m in &libs {
                lib_part.push_str(&gtab(&em, &m.name));
            }
            lib_part.push_str(&ic_statics());
            IC_LIB_END.with(|c| c.set(IC_COUNT.with(|n| n.get())));
            lib_part.push_str(LIB_END);
        }
    }
    let mut out = lib_part;
    out.push_str(&em.items);
    out.push_str(&inits);
    // user module attribute hook
    let mut hook = String::from("fn user_mod(m: &str, n: &str) -> Option<R> { match m { ");
    for m in mods.iter().skip(1) {
        write!(hook, "{} => {{ match n {{ ", lit(&m.name)).unwrap();
        let names = em.mod_names[&m.name].clone();
        for nm in names.iter().cloned() {
            let i = em.gslot(&m.name, &nm);
            write!(hook, "{} => Some(gl({i}, {})), ", lit(&nm), lit(&nm)).unwrap();
        }
        for nm in ["__name__", "__file__", "__doc__", "__package__", "__spec__", "__loader__", "__cached__", "__builtins__", "__dict__", "__path__"] {
            if names.contains(nm) {
                continue;
            }
            match em.mod_dunder(&m.name, nm) {
                Some(Ok(v)) => write!(hook, "{} => Some(Ok({v})), ", lit(nm)).unwrap(),
                Some(Err(())) => write!(hook, "{} => Some(Err(unsupported({}))), ", lit(nm), lit(&format!("{}.{nm}", m.name))).unwrap(),
                None => {}
            }
        }
        write!(hook, "_ => Some(Err(exc(\"AttributeError\", format!(\"module '{{}}' has no attribute '{{}}'\", m, n)))) }} }} ").unwrap();
    }
    hook.push_str("_ => None } }\n");
    out.push_str(&hook);
    // the program's files by module name, for a module's repr
    let mut files = String::from("fn user_mod_file(m: &str) -> Option<&'static str> { match m { ");
    for m in mods.iter() {
        if let Some(p) = &m.path {
            write!(files, "{} => Some({}), ", lit(&m.name), lit(p)).unwrap();
        }
    }
    files.push_str("_ => None } }\n");
    out.push_str(&files);
    // name -> global slot, per module, for globals()
    for m in mods.iter().filter(|m| !libs.iter().any(|l| l.name == m.name)) {
        out.push_str(&gtab(&em, &m.name));
    }
    let lone: HashMap<usize, &String> = em.lone_idx.iter().map(|(h, &i)| (i, h)).collect();
    let consts: Vec<String> = em.consts.iter().enumerate().map(|(i, c)| match lone.get(&i) {
        Some(h) => format!("lone_lit({})", lit(h)),
        None => format!("s({})", lit(c)),
    }).collect();
    // re's \N{NAME}: the name table goes into programs whose source may hold such a pattern
    let uname = em.uses_re && em.bridge.srcs.values().any(|(_, t)| t.contains("\\N{"));
    // At exit CPython finalizes __main__'s globals in the order they were first set; the slots are in name order,
    // so the order is the names' first appearance in the module's source.
    let del_on = if em.has_del {
        let main = &mods[0];
        let src = format!("{:?}", main.body);
        let mut order: Vec<(usize, usize)> = em
            .mod_names
            .get(&main.name)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|n| (src.find(&format!("{:?}", n)).unwrap_or(usize::MAX), em.gslot(&main.name, n)))
            .collect();
        order.sort();
        let idx: Vec<String> = order.iter().map(|(_, i)| i.to_string()).collect();
        format!("set_del_on(vec![{}]);", idx.join(", "))
    } else {
        String::new()
    };
    write!(
        out,
        "fn __run() -> Result<(), E> {{ unsafe {{ GL = vec![V::Unbound; {}]; K = vec![{}]; }} USER_MOD.with(|u| u.set(Some(user_mod))); USER_MOD_FILE.with(|u| u.set(Some(user_mod_file))); MAIN_GTAB.with(|u| u.set(Some(gtab___main__))); {}{}{} let r = init___main__(); fin_point(); r }}\nfn main() {{ run_main(__run); }}\n",
        em.nglobals.max(1),
        consts.join(", "),
        del_on,
        // the program uses CPython's warnings module: the warnings the runtime issues go through it
        if em.bridged_used.contains("warnings") { " set_warn_bridge(pyd_warn_at); pyd_warn_install()?;" } else { "" },
        if uname { " set_name_lookup(__uname);" } else { "" }
    )
    .unwrap();
    if uname {
        out.push_str(include_str!("uname_glue.rs"));
        out.push_str("const UNAMES: &str = r###\"");
        out.push_str(include_str!("../pynames/unicode_names.txt"));
        out.push_str("\"###;\n");
    }
    let _ = &em.out;
    // a program using warnings: each function puts its caller's line back when it returns (or raises)
    let out = if em.track_warn { out.replace("let __depth = Depth::new()?;", "let __depth = Depth::new()?; let __wg = warn_guard();") } else { out };
    // CPython's re matches any buffer (array.array, mmap, ctypes arrays) as bytes; Brevi's re takes a
    // library object's buffer for array.array, but an mmap's buffer changes under the match and ctypes was
    // not checked. weakref would be handed Brevi's Pattern and Match values, which a bridged library
    // cannot hold. CPython runs such a program.
    if em.uses_re {
        if let Some(m) = em.bridged_used.iter().find(|m| matches!(m.as_str(), "mmap" | "ctypes" | "weakref")) {
            return Err((mods[0].name.clone(), 1, format!("re together with {m} is not supported in dynamic builds")));
        }
    }
    // mmap and ctypes arrays are buffers CPython's bytearray and memoryview take (memoryview(x), ba.extend(x),
    // x.write(ba)); array.array is one too, but its buffer is read through the bridge (rt_bytes::bytes_of) and a
    // memoryview of it is refused at run time, so bytearray and memoryview may appear together with array.
    if em.bridge.srcs.values().any(|(_, t)| t.contains("bytearray") || t.contains("memoryview")) {
        if let Some(m) = em.bridged_used.iter().find(|m| matches!(m.as_str(), "mmap" | "ctypes")) {
            return Err((mods[0].name.clone(), 1, format!("bytearray or memoryview together with {m} is not supported in dynamic builds")));
        }
    }
    Ok((out, em.bridged_used.into_iter().collect()))
}

// The program's Rust up to this line is its library copies (see program).
pub const LIB_END: &str = "\n//==BREVI_LIB_END==\n";

// name -> global slot in module `m`, for globals()
fn gtab(em: &Em, m: &str) -> String {
    let mut slots: Vec<(String, usize)> = em.globals.iter().filter(|((mm, n), _)| mm == m && !n.starts_with('<')).map(|((_, n), i)| (n.clone(), *i)).collect();
    slots.sort();
    let mut out = format!("#[allow(dead_code)] fn gtab_{}(k: &str) -> Option<usize> {{ match k {{ ", rust_ident(m));
    for (n, i) in slots {
        write!(out, "{} => Some({i}), ", lit(&n)).unwrap();
    }
    out.push_str("_ => None } }\n");
    out
}

// fields used by the code above
impl Em {
    #[allow(dead_code)]
    fn _unused(&self) {}
}

// The number literals of eval() text, checked by Python's own token rules (the dynamic parser is lenient about
// them): None when one is invalid, Some(false) when one is imaginary, else Some(true).
fn eval_numbers(src: &str) -> Option<bool> {
    let c: Vec<char> = src.chars().collect();
    let idc = |x: char| x.is_alphanumeric() || x == '_';
    let digits = |j: &mut usize, ok: &dyn Fn(char) -> bool| -> bool {
        // d(_?d)*: false when there is no digit; stops before a '_' not followed by a digit
        if *j >= c.len() || !ok(c[*j]) {
            return false;
        }
        *j += 1;
        loop {
            if *j < c.len() && ok(c[*j]) {
                *j += 1;
            } else if *j + 1 < c.len() && c[*j] == '_' && ok(c[*j + 1]) {
                *j += 2;
            } else {
                return true;
            }
        }
    };
    let mut plain = true;
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\'' || ch == '"' {
            let mut j = i + 1;
            while j < c.len() && c[j] != ch {
                j += if c[j] == '\\' { 2 } else { 1 };
            }
            i = j + 1;
            continue;
        }
        let starts = ch.is_ascii_digit() || (ch == '.' && i + 1 < c.len() && c[i + 1].is_ascii_digit());
        if !starts || (i > 0 && idc(c[i - 1])) {
            i += 1;
            continue;
        }
        let mut j = i;
        if ch == '0' && i + 1 < c.len() && "xXoObB".contains(c[i + 1]) {
            j += 2;
            let r = c[i + 1].to_ascii_lowercase();
            let ok: &dyn Fn(char) -> bool = match r {
                'x' => &|x: char| x.is_ascii_hexdigit(),
                'o' => &|x: char| ('0'..='7').contains(&x),
                _ => &|x: char| x == '0' || x == '1',
            };
            if j < c.len() && c[j] == '_' {
                j += 1;
            }
            if !digits(&mut j, ok) {
                return None;
            }
        } else {
            let dec = |x: char| x.is_ascii_digit();
            let int_start = j;
            let had_int = digits(&mut j, &dec);
            let int_end = j;
            let mut is_float = false;
            if j < c.len() && c[j] == '.' {
                is_float = true;
                j += 1;
                let mut k = j;
                if digits(&mut k, &dec) {
                    j = k;
                }
            }
            if (had_int || is_float) && j < c.len() && (c[j] == 'e' || c[j] == 'E') {
                let mut k = j + 1;
                if k < c.len() && (c[k] == '+' || c[k] == '-') {
                    k += 1;
                }
                if digits(&mut k, &dec) {
                    j = k;
                    is_float = true;
                }
            }
            if j < c.len() && (c[j] == 'j' || c[j] == 'J') {
                j += 1;
                plain = false;
            } else if !is_float {
                let t: String = c[int_start..int_end].iter().filter(|x| **x != '_').collect();
                if t.len() > 1 && t.starts_with('0') && t.chars().any(|x| x != '0') {
                    return None;
                }
            }
        }
        if j < c.len() && idc(c[j]) {
            let mut k = j;
            while k < c.len() && idc(c[k]) {
                k += 1;
            }
            let w: String = c[j..k].iter().collect();
            if !matches!(w.as_str(), "and" | "else" | "for" | "if" | "in" | "is" | "not" | "or") {
                return None;
            }
        }
        i = j;
    }
    Some(plain)
}


// The special methods a metaclass may define (rt_call.rs keeps the same list and checks it at run time).
const META_DUNDERS: &[&str] = &[
    "__module__", "__qualname__", "__doc__", "__new__", "__init__", "__prepare__", "__lt__", "__le__", "__gt__", "__ge__", "__eq__", "__ne__",
    "__hash__", "__len__", "__bool__", "__repr__", "__str__", "__annotations__", "__init_subclass__", "__weakref__", "__dict__",
];

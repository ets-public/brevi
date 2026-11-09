// Emits Rust for a parsed Python program, against the dynamic runtime (rt_*.rs).
use super::parse::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

type ER<T> = Result<T, (usize, String)>;

pub const BUILTIN_MODULES: [&str; 49] = ["_brevi_srv", "_brevi_proc", "importlib.metadata", "zoneinfo", "tzlocal", "atexit", "_brevi_async", "_brevi_net", "_brevi_fs", "_brevi_st", "_brevi_ort", "_brevi_sqlite", "_brevi_duckdb", "_brevi_stdio", "_brevi_tok", "_brevi_hf", "_brevi_mt", "_brevi_term", "hashlib", "base64", "binascii", "json.decoder", "enum", "sys", "math", "collections", "heapq", "bisect", "functools", "itertools", "copy", "operator", "string", "time", "typing", "dataclasses", "abc", "re", "json", "__future__", "decimal", "fractions", "datetime", "csv", "html", "statistics", "threading", "gc", "weakref"];


// ---------- numpy, natively (rt_np*.rs) ----------
// Set while a build uses Brevi's own numpy (on until a refusal names numpy; then CPython's numpy is used).
pub static NP_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
// Set while the program being emitted imports numpy natively.
pub static NP_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// The program uses Brevi's pydantic: defs carry their number of positional parameters (src/dynpy/pyd.rs).
pub static PYD_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
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
// collections.abc as Brevi's typing forms (off when the program needs CPython's ABCs: the bridge runs them).
pub static ABC_NATIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn builtin_mod(m: &str) -> bool {
    BUILTIN_MODULES.contains(&m) || m == "typing_extensions" || (m == "collections.abc" && ABC_NATIVE.load(std::sync::atomic::Ordering::Relaxed)) || (matches!(m, "numpy" | "numpy.random" | "numpy.linalg") && np_native())
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
thread_local! {
    static IN_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn np_attr_check(n: &str, kws: &[&str], line: usize) -> ER<()> {
    if !np_on() || !NP_ATTR_ALL.contains(&n) || IN_COPY.with(|c| c.get()) {
        return Ok(());
    }
    if n == "metadata" && NP_ATTR_ST.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    if np_narrow() && matches!(n, "dot" | "trace") {
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
        "typing" | "typing_extensions" | "collections.abc" => return super::typing::module_has(m, n),
        "numpy" => return np_func_kw(n).is_some() && (!np_narrow() || NP_NARROW_FUNCS.contains(&n)),
        "numpy.random" => return np_rand_kw(n).is_some(),
        "numpy.linalg" => return np_linalg_kw(n).is_some() && !np_narrow(),
        "sys" => &["exit", "setrecursionlimit", "getrecursionlimit", "getsizeof", "intern", "set_int_max_str_digits", "stdin", "stdout", "stderr", "argv", "maxsize", "platform", "byteorder", "version_info", "version", "modules", "path", "exception", "exc_info"],
        "math" => &["acos", "asin", "atan", "atan2", "ceil", "comb", "copysign", "cos", "cosh", "degrees", "dist", "exp", "fabs", "factorial", "floor", "fmod", "frexp", "fsum", "gcd", "hypot", "isclose", "isfinite", "isinf", "isnan", "isqrt", "lcm", "ldexp", "log", "log10", "log1p", "log2", "modf", "perm", "pow", "prod", "radians", "remainder", "sin", "sinh", "sqrt", "tan", "tanh", "trunc", "pi", "e", "tau", "inf", "nan"],
        "atexit" => &["register", "unregister", "_run_exitfuncs", "_clear", "_ncallbacks"],
        "heapq" => &["heapify", "heappop", "heappush", "heappushpop", "heapreplace", "merge", "nlargest", "nsmallest"],
        "bisect" => &["bisect", "bisect_left", "bisect_right", "insort", "insort_left", "insort_right"],
        "functools" => &["cache", "cmp_to_key", "lru_cache", "partial", "reduce", "total_ordering", "wraps"],
        "itertools" => &["accumulate", "chain", "combinations", "combinations_with_replacement", "compress", "count", "cycle", "dropwhile", "filterfalse", "groupby", "islice", "pairwise", "permutations", "product", "repeat", "starmap", "takewhile", "zip_longest"],
        "copy" => &["copy", "deepcopy"],
        "operator" => &["add", "and_", "attrgetter", "contains", "eq", "floordiv", "ge", "getitem", "gt", "index", "itemgetter", "le", "lshift", "lt", "methodcaller", "mod", "mul", "ne", "neg", "not_", "or_", "pow", "rshift", "sub", "truediv", "truth", "xor"],
        "string" => &["ascii_lowercase", "ascii_uppercase", "ascii_letters", "digits", "hexdigits", "octdigits", "punctuation", "whitespace", "printable"],
        "time" => &["time", "perf_counter", "monotonic", "process_time", "sleep", "time_ns", "perf_counter_ns", "monotonic_ns", "process_time_ns", "gmtime", "strftime"],
        "abc" => &["ABC", "abstractmethod"],
        "enum" => &["Enum", "IntEnum", "StrEnum", "Flag", "IntFlag", "auto", "unique"],
        "_brevi_term" => &["columns"],
        "hashlib" => &["new", "md5", "sha1", "sha224", "sha256", "sha384", "sha512"],
        "base64" => &["b64encode", "b64decode", "standard_b64encode", "standard_b64decode", "urlsafe_b64encode", "urlsafe_b64decode", "encodebytes", "decodebytes", "b16encode", "b16decode"],
        "binascii" => &["Error", "hexlify", "unhexlify", "b2a_hex", "a2b_hex"],
        "_brevi_async" => &["suspend", "iscoroutine", "isawaitable", "isasyncgen", "set_hooks", "track", "untrack", "tracked"],
        "_brevi_stdio" => &["claim", "release", "read", "write", "poll"],
        "_brevi_proc" => &["spawn", "read", "write", "close", "wait", "signal", "poll"],
        "_brevi_srv" => &["listen", "accept", "read", "write", "close", "poll", "random"],
        "_brevi_fs" => &["read", "write", "read_at", "size", "write_bytes", "listdir", "stat", "load", "save"],
        "_brevi_ort" => &["load", "providers", "build_info", "session", "run", "free"],
        "_brevi_st" => &["header", "header_len", "tensor", "dtype_name", "slice", "serialize"],
        "_brevi_sqlite" => &[
            "load", "open", "close", "errmsg", "errcode", "errstr", "prepare", "finalize", "reset", "clear", "step", "bind", "nparams", "param_name",
            "ncols", "col_name", "col_decl", "row", "col", "run", "fetch", "fetch_all", "db_int", "stmt_int", "busy_timeout", "interrupt", "limit", "complete",
        ],
        "_brevi_duckdb" => &["load", "open", "connect", "disconnect", "close", "query", "many", "describe", "cols", "info", "fetch", "free", "interrupt", "term"],
        "_brevi_tok" => &["core", "ranks", "max_rank", "n_ranks", "find_special"],
        "_brevi_net" => &["open", "open_nb", "ready", "tls", "handshake", "settimeout", "setblocking", "send", "sendall", "fill", "head", "line", "take", "avail", "fileno", "close", "poll", "env", "host_ok", "zopen", "zfeed", "zstate", "zrest", "zclose"],
        "_brevi_hf" => &[
            "load", "load_built", "encode", "encode_batch", "decode", "decode_batch", "token_to_id", "id_to_token", "vocab", "vocab_size",
            "n_added", "set_truncation", "truncation", "set_padding", "padding", "add_tokens", "set_encode_special", "encode_special",
            "added_tokens", "lacks",
        ],
        "_brevi_mt" => &["new", "seed", "next", "random", "bits", "below", "shuffle"],
        "collections" => &["defaultdict", "Counter", "OrderedDict", "deque", "namedtuple"],
        "dataclasses" => &["asdict", "astuple", "dataclass", "field", "fields", "replace", "is_dataclass", "MISSING", "KW_ONLY", "InitVar", "Field", "FrozenInstanceError"],
        "re" => &["compile", "escape", "findall", "finditer", "fullmatch", "match", "purge", "search", "split", "sub", "subn", "IGNORECASE", "I", "MULTILINE", "M", "DOTALL", "S", "VERBOSE", "X", "ASCII", "A", "error"],
        "json" => &["dump", "dumps", "load", "loads", "JSONDecodeError", "JSONDecoder", "JSONEncoder"],
        "json.decoder" => &["scanstring", "JSONDecoder", "JSONDecodeError"],
        "decimal" => &["Decimal", "getcontext", "InvalidOperation", "DivisionByZero", "DecimalException", "ROUND_HALF_EVEN", "ROUND_HALF_UP", "ROUND_HALF_DOWN", "ROUND_UP", "ROUND_DOWN", "ROUND_CEILING", "ROUND_FLOOR", "ROUND_05UP"],
        "fractions" => &["Fraction"],
        "datetime" => &["date", "datetime", "time", "timedelta", "timezone", "UTC", "MINYEAR", "MAXYEAR"],
        "csv" => &["DictReader", "DictWriter", "reader", "writer", "QUOTE_MINIMAL", "QUOTE_ALL", "QUOTE_NONNUMERIC", "QUOTE_NONE", "Error"],
        "statistics" => &["fmean", "mean", "median", "median_high", "median_low", "mode", "multimode", "pstdev", "pvariance", "stdev", "variance", "StatisticsError"],
        "threading" => &["Lock", "RLock", "Thread", "active_count", "current_thread", "main_thread", "stack_size"],
        "html" => &["escape", "unescape"],
        "tzlocal" => &["get_localzone_name", "get_localzone"],
        "importlib.metadata" => &["version", "PackageNotFoundError"],
        "zoneinfo" => &["ZoneInfo", "ZoneInfoNotFoundError", "available_timezones", "TZPATH", "reset_tzpath"],
        "gc" => &["collect"],
        "weakref" => &["ref"],
        _ => return false,
    };
    names.contains(&n)
}
// Python builtins dynamic builds do not have: refused when the program is built.
fn is_missing_builtin(n: &str) -> bool {
    matches!(n, "exec" | "eval" | "compile" | "__import__" | "bytearray" | "memoryview" | "complex" | "dir" | "help" | "breakpoint" | "open" | "__build_class__" | "globals" | "locals")
}

fn is_builtin_name(n: &str) -> bool {
    matches!(
        n,
        "print" | "input" | "len" | "repr" | "ascii" | "abs" | "min" | "max" | "sum" | "sorted" | "reversed" | "enumerate" | "zip" | "map" | "filter" | "any" | "all" | "iter" | "next" | "isinstance" | "issubclass" | "hasattr" | "getattr" | "setattr" | "delattr" | "callable" | "ord" | "chr" | "hex" | "oct" | "bin" | "round" | "divmod" | "pow" | "hash" | "id" | "format" | "vars" | "exit" | "quit" | "staticmethod" | "classmethod" | "property" | "int" | "float" | "str" | "bool" | "list" | "tuple" | "dict" | "set" | "frozenset" | "object" | "type" | "range" | "slice" | "super" | "NotImplemented" | "Ellipsis" | "__name__" | "bytes" | "__bytes_lit" | "aiter" | "anext" | "__brevi_aenter" | "__brevi_aexit" | "__brevi_eg_split" | "__brevi_eg_reraise"
    ) || is_exc_name(n)
}
fn is_exc_name(n: &str) -> bool {
    matches!(
        n,
        "BaseException" | "Exception" | "SystemExit" | "KeyboardInterrupt" | "GeneratorExit" | "ArithmeticError" | "ZeroDivisionError" | "OverflowError" | "FloatingPointError" | "LookupError" | "KeyError" | "IndexError" | "ValueError" | "UnicodeError" | "UnicodeDecodeError" | "UnicodeEncodeError" | "TypeError" | "AttributeError" | "NameError" | "UnboundLocalError" | "RuntimeError" | "RecursionError" | "NotImplementedError" | "StopIteration" | "StopAsyncIteration" | "TimeoutError" | "ConnectionError" | "ConnectionRefusedError" | "ConnectionResetError" | "ConnectionAbortedError" | "BrokenPipeError" | "BaseExceptionGroup" | "ExceptionGroup" | "AssertionError" | "ImportError" | "ModuleNotFoundError" | "OSError" | "IOError" | "EnvironmentError" | "FileNotFoundError" | "PermissionError" | "IsADirectoryError" | "NotADirectoryError" | "FileExistsError" | "EOFError" | "MemoryError" | "BufferError" | "Warning" | "UserWarning" | "DeprecationWarning"
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
            St::AnnAssign(t, v, _) => {
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
            }
            _ => {}
        }
    }
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
fn scope_of(body: &[Stmt], params: &[String]) -> Scope {
    let mut sc = Scope::default();
    for p in params {
        sc.locals.insert(p.clone());
    }
    fn walk(body: &[Stmt], sc: &mut Scope) {
        for s in body {
            match &s.st {
                St::Assign(ts, _) => ts.iter().for_each(|t| target_names(t, &mut sc.locals)),
                St::AugAssign(t, _, _) | St::AnnAssign(t, _, _) => target_names(t, &mut sc.locals),
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
    used.into_iter().filter(|n| !sc.locals.contains(n) || sc.nonlocals.contains(n)).filter(|n| !sc.globals.contains(n)).collect()
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
        });
    }
    out
}
// Generators switch stacks with a few lines of assembly (rt_gen.rs), written for these targets.
const GEN_OK: bool = cfg!(all(any(target_arch = "x86_64", target_arch = "aarch64"), not(windows)));

// The line of a yield inside a try that has a finally, or inside a with block.
fn yield_in_cleanup(body: &[Stmt], inside: bool) -> Option<usize> {
    for s in body {
        let here = |b: &[Stmt]| -> Option<usize> { if has_yield(b) { Some(s.line) } else { None } };
        let found = match &s.st {
            St::Try(b, hs, e, f) => {
                let guarded = f.is_some() || inside;
                let mut r = yield_in_cleanup(b, guarded);
                for h in hs {
                    r = r.or_else(|| yield_in_cleanup(&h.body, guarded));
                }
                if let Some(e) = e {
                    r = r.or_else(|| yield_in_cleanup(e, guarded));
                }
                if let Some(f) = f {
                    r = r.or_else(|| here(f));
                }
                r
            }
            St::With(_, b) => here(b),
            St::If(arms, els) => arms.iter().find_map(|(_, b)| yield_in_cleanup(b, inside)).or_else(|| els.as_ref().and_then(|b| yield_in_cleanup(b, inside))),
            St::While(_, b, e) | St::For(_, _, b, e) => yield_in_cleanup(b, inside).or_else(|| e.as_ref().and_then(|b| yield_in_cleanup(b, inside))),
            _ if inside => here(std::slice::from_ref(s)),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

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
                St::AnnAssign(t, Some(v), _) if refers(v, &names, &attrs) => grew |= bind(t, &mut names, &mut attrs),
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
    exc_vars: Vec<String>,
    is_module: bool,
    is_class: bool,
    // an async def's body: await may be used
    is_async: bool,
    // an async def with yield: an async generator
    is_agen: bool,
    // prefix of the names of the cells this function owns ("k{id}_"); empty for module and class bodies
    cells: String,
}

pub struct Module {
    pub name: String,
    pub body: Vec<Stmt>,
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
}
impl Bridge {
    pub fn has(&self, m: &str) -> bool {
        self.on && self.modules.contains(m.split('.').next().unwrap_or(m))
    }
}

pub struct Em {
    // cells with a CellGuard (see self_recursive_def)
    guards: HashSet<String>,
    out: String,
    items: String,
    n: usize,
    globals: HashMap<(String, String), usize>,
    nglobals: usize,
    consts: Vec<String>,
    const_idx: HashMap<String, usize>,
    fstack: Vec<Func>,
    module: String,
    user_modules: Vec<String>,
    // module-level defs that can be called directly: (module, name) -> (rust fn, nparams, default slots, defaults count)
    direct: HashMap<(String, String), (String, usize, Vec<usize>)>,
    // module-level names bound only once by import of a builtin module
    mod_alias: HashMap<(String, String), String>,
    // names assigned anywhere at module level, per module
    mod_names: HashMap<String, BTreeSet<String>>,
    class_ctx: Option<String>,
    // the def being built is an async def (read by closure)
    next_async: bool,
    direct_ok: HashSet<(String, String)>,
    bridge: Bridge,
    // what the program has CPython run, for the path report
    bridged_used: BTreeSet<String>,
}

impl Em {
    fn id(&mut self) -> usize {
        self.n += 1;
        self.n
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
    fn f(&mut self) -> &mut Func {
        self.fstack.last_mut().unwrap()
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
        match self.lookup(name) {
            Some(Var::Local(r)) => Ok(self.q(&format!("ld(&{r}, {})", lit(name)))),
            Some(Var::Cell(r)) => {
                let own = !self.f().cells.is_empty() && r.starts_with(&self.f().cells);
                Ok(self.q(&format!("{}(&{r}, {})", if own { "cget_own" } else { "cget" }, lit(name))))
            }
            Some(Var::Global(i)) => {
                if is_builtin_name(name) || name == "__name__" {
                    Ok(self.q(&format!("glb({i}, {})", lit(name))))
                } else {
                    Ok(self.q(&format!("gl({i}, {})", lit(name))))
                }
            }
            None => {
                if name == "__name__" {
                    return Ok(self.konst(&self.module.clone()).replace("kc", "kc"));
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
        }
    }
    fn store(&mut self, name: &str, val: &str) -> String {
        match self.lookup(name) {
            Some(Var::Local(r)) => format!("{r} = {val};"),
            Some(Var::Cell(r)) => format!("cset(&{r}, {val});"),
            Some(Var::Global(i)) => format!("gs({i}, {val});"),
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
            Ex::NoneV => "V::None".into(),
            Ex::True => "V::Bool(true)".into(),
            Ex::False => "V::Bool(false)".into(),
            Ex::Ellipsis => "V::Ellipsis".into(),
            Ex::FStr(parts) => {
                let body = self.fparts(parts, line)?;
                format!("st({{ let mut __s = String::new(); {body} __s }})")
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
                let s = self.seq(v, line)?;
                // CPython builds a literal of 3 or more constants as a frozenset constant copied into a new
                // set, which can order the items differently from adding them one by one.
                fn konst(e: &Ex) -> bool {
                    match e {
                        Ex::Int(_) | Ex::Float(_) | Ex::Str(_) | Ex::NoneV | Ex::True | Ex::False => true,
                        Ex::Unary("-", x) => matches!(x.as_ref(), Ex::Int(_) | Ex::Float(_)),
                        Ex::Tuple(xs) => xs.iter().all(konst),
                        _ => false,
                    }
                }
                let one = self.q(&format!("set_of_vec({s})"));
                if v.len() >= 3 && v.iter().all(konst) {
                    let fs = self.q(&format!("bi_frozenset(Args::p(vec![{one}]))"));
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
            Ex::Attr(o, n) => {
                if let Some(m) = self.static_module(o) {
                    if self.user_modules.contains(&m) {
                        return self.user_mod_load(&m, n, line);
                    }
                    if !module_has(&m, n) {
                        return Err(err(line, &format!("{m}.{n} is not supported in dynamic builds")));
                    }
                    return Ok(self.q(&format!("module_attr({}, {})", lit(&m), lit(n))));
                }
                np_attr_check(n, &[], line)?;
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
                if *op == "@" && np_on() && np_narrow() {
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
                let ret = Stmt { line, st: St::Return(Some((**body).clone())) };
                let f = self.closure("<lambda>", p, &[ret], line, false)?;
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
                    let ee = self.ex(e, line)?;
                    let v = match conv {
                        Some('r') => format!("st({})", self.q(&format!("repr(&{ee})"))),
                        Some('s') => format!("st({})", self.q(&format!("to_str(&{ee})"))),
                        Some('a') => self.q(&format!("bi_ascii(Args::p(vec![{ee}]))")),
                        _ => ee,
                    };
                    if spec.is_empty() {
                        let t = self.q(&format!("to_str(&__x)"));
                        write!(b, "{{ let __x = {v}; if let V::Str(__y) = &__x {{ __s.push_str(&__y.s); }} else {{ __s.push_str(&{t}); }} }} ").unwrap();
                    } else {
                        let sp = self.fparts(spec, line)?;
                        let fs = self.q("format_spec(&__x, &__spec)");
                        write!(b, "{{ let __x = {v}; let __spec = {{ let mut __s = String::new(); {sp} __s }}; __s.push_str(&{fs}); }} ").unwrap();
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
    fn user_mod_load(&mut self, m: &str, n: &str, line: usize) -> ER<String> {
        if !self.mod_names.get(m).is_some_and(|s| s.contains(n)) {
            return Ok(self.q(&format!("Err::<V, E>(exc(\"AttributeError\", {}.into()))", lit(&format!("module '{m}' has no attribute '{n}'")))));
        }
        let i = self.gslot(m, n);
        let _ = line;
        Ok(self.q(&format!("gl({i}, {})", lit(n))))
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
                        let cls = self.load_raw(&cls, line)?;
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
                let a = self.args(args, line, None)?;
                if !module_has(&md, m) {
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
                return Ok(self.q(&format!("module_call({}, {}, {a})", lit(&md), lit(m))));
            }
            let kws: Vec<&str> = args.iter().filter_map(|x| if let Arg::Kw(k, _) = x { Some(k.as_str()) } else { None }).collect();
            np_attr_check(m, &kws, line)?;
            let oo = self.ex(o, line)?;
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
                    return Ok(self.q(&format!("{fname}({a})")));
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
    fn comp(&mut self, kind: char, elt: &Ex, val: Option<&Ex>, gens: &[CompFor], line: usize) -> ER<String> {
        // `async for` clauses and `await`s: the comprehension runs in place in the async function (CPython inlines
        // it too). A generator expression with them is an async generator, which Brevi builds eagerly: refused.
        let mut awaits = gens.iter().any(|g| g.is_async);
        visit_ex(&Ex::Comp(kind, Box::new(elt.clone()), val.map(|v| Box::new(v.clone())), gens.to_vec()), &mut |x| {
            if matches!(x, Ex::Await(_)) {
                awaits = true;
            }
        });
        if awaits {
            if kind == 'g' {
                return Err(err(line, "asynchronous generator expressions are not supported in dynamic builds"));
            }
            if !self.fstack.last().unwrap().is_async {
                return Err(err(line, "asynchronous comprehension outside of an asynchronous function"));
            }
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
        let mut decl = String::new();
        for n in &names {
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
            'g' => b.push_str("vec_iter(__out) }"),
            's' => b.push_str("V::Set(Rc::new(RefCell::new(__out))) }"),
            _ => b.push_str("V::Dict(Rc::new(RefCell::new(__out))) }"),
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
        for s in body {
            b.push_str(&self.stmt(s)?);
            b.push('\n');
        }
        Ok(b)
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
            St::AnnAssign(t, v, _) => match v {
                Some(v) => {
                    let vv = self.ex(v, line)?;
                    self.assign(t, &vv, line)?
                }
                None => String::new(),
            },
            St::AugAssign(t, op, v) => {
                if *op == "@" && np_on() && np_narrow() {
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
                for t in ts {
                    match t {
                        Ex::Name(n) => match self.lookup(n) {
                            Some(Var::Local(r)) => write!(b, "{r} = V::Unbound; ").unwrap(),
                            Some(Var::Cell(r)) => write!(b, "cset(&{r}, V::Unbound); ").unwrap(),
                            Some(Var::Global(i)) => write!(b, "gs({i}, V::Unbound); ").unwrap(),
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
                    if self.user_modules.contains(&target) {
                        let init = self.q(&format!("init_{}()", rust_ident(&target)));
                        write!(b, "{init}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("V::Module({})", lit(&target))));
                    } else if self.bridge.has(m) {
                        self.bridged_used.insert(m.split('.').next().unwrap_or(m).to_string());
                        let v = self.q(&format!("pyd_import({}, {})", lit(m), a.is_none()));
                        b.push_str(&self.store(&bind, &v));
                    } else if builtin_mod(target.as_str()) || matches!(target.as_str(), "os.path") {
                        b.push_str(&self.store(&bind, &format!("V::Module({})", lit(&target))));
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
                let user = self.user_modules.contains(m);
                if !user && !builtin_mod(m.as_str()) && self.bridge.has(m) {
                    self.bridged_used.insert(m.split('.').next().unwrap_or(m).to_string());
                    for (n, a) in names {
                        if n == "*" {
                            return Err(err(line, "from ... import * is not supported for library modules"));
                        }
                        let v = self.q(&format!("pyd_from({}, {})", lit(m), lit(n)));
                        b.push_str(&self.store(a.as_ref().unwrap_or(n), &v));
                    }
                    return Ok(b);
                }
                if user {
                    let init = self.q(&format!("init_{}()", rust_ident(m)));
                    write!(b, "{init}; ").unwrap();
                } else if !builtin_mod(m.as_str()) {
                    return Err(err(line, &format!("module '{m}' is not supported in dynamic builds")));
                }
                for (n, a) in names {
                    if n == "*" {
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
                    if !user && !module_has(m, n) {
                        return Err(err(line, &format!("{m}.{n} is not supported in dynamic builds")));
                    }
                    let sub = format!("{m}.{n}");
                    if user && self.user_modules.contains(&sub) && (self.module == *m || !self.mod_names.get(m).is_some_and(|s| s.contains(n))) {
                        // `from pkg import sub`: the package's module sub
                        let init = self.q(&format!("init_{}()", rust_ident(&sub)));
                        write!(b, "{init}; ").unwrap();
                        b.push_str(&self.store(&bind, &format!("V::Module({})", lit(&sub))));
                        continue;
                    }
                    let v = if user { self.user_mod_load(m, n, line)? } else { self.q(&format!("module_attr({}, {})", lit(m), lit(n))) };
                    b.push_str(&self.store(&bind, &v));
                }
                b
            }
            St::If(arms, els) if arms.len() == 1 && els.is_none() && super::typing::type_checking(&arms[0].0) => String::new(),
            St::If(arms, els) => {
                let mut b = String::new();
                for (k, (c, body)) in arms.iter().enumerate() {
                    let cc = self.cond(c, line)?;
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
                let cc = self.cond(c, line)?;
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
                // `__del__` runs when CPython frees the object (reference counts), which dynamic builds do not track:
                // the program's output could differ, so CPython runs it.
                if let Some(d) = c.body.iter().find(|s| matches!(&s.st, St::Def(d) if d.name == "__del__")) {
                    return Err(err(d.line, "__del__ is not supported in dynamic builds"));
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
            let nx = self.q(&format!("__it{id}.next()"));
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
        let exit_ok = self.q(&format!("call_method(&__m{id}, \"__exit__\", Args::p(vec![V::None, V::None, V::None]))"));
        let exit_err = self.q(&format!("call_method(&__m{id}, \"__exit__\", Args::p(vec![V::Class(type_of(&__ev)), __ev.clone(), V::None]))"));
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
            // CPython runs a finally (or a with block's exit) when a generator left part way is dropped;
            // here a dropped generator just stops.
            if let Some(l) = yield_in_cleanup(&d.body, false) {
                return Err(err(l, "a yield inside try/finally or with is not supported in dynamic builds"));
            }
        }
        if d.is_async && !GEN_OK {
            return Err(err(d.line, "async functions are not supported in dynamic builds on this platform"));
        }
        self.next_async = d.is_async;
        let v = self.closure(&d.name, &d.params, &d.body, d.line, true);
        self.next_async = false;
        let mut v = v?;
        if let Some(doc) = docstring(&d.body) {
            v = format!("with_doc({v}, {})", lit(doc));
        }
        if PYD_ON.load(std::sync::atomic::Ordering::Relaxed) {
            let n = if d.params.star.is_some() { 99 } else { d.params.pos.len() };
            v = format!("with_np({v}, {n})");
        }
        for dec in d.decorators.iter().rev() {
            let dd = self.ex(dec, d.line)?;
            v = self.q(&format!("call(&{dd}, Args::p(vec![{v}]))"));
        }
        Ok(v)
    }
    // A function value. Module-level functions (and methods of module-level classes) become plain Rust
    // functions plus a wrapper; nested ones are closures capturing the cells they use.
    fn closure(&mut self, name: &str, p: &Params, body: &[Stmt], line: usize, is_def: bool) -> ER<String> {
        let id = self.id();
        let is_async = std::mem::take(&mut self.next_async);
        let is_gen = has_yield(body);
        let params = lambda_params(p);
        let sc = scope_of(body, &params);
        let inner_free = nested_free(body);
        let at_top = self.fstack.len() == 1;
        let in_class_top = self.class_ctx.is_some() && self.fstack.len() == 2 && self.fstack[0].is_module;
        // defaults, evaluated now
        let mut dflt_codes = vec![];
        for (_, dv) in p.pos.iter() {
            dflt_codes.push(match dv {
                Some(x) => self.ex(x, line)?,
                None => "V::Unbound".into(),
            });
        }
        if p.star.is_some() {
            // no default
        }
        let mut kwd = vec![];
        for (_, dv) in p.kwonly.iter() {
            kwd.push(match dv {
                Some(x) => self.ex(x, line)?,
                None => "V::Unbound".into(),
            });
        }
        // signature: positional params, then kw-only params share the names list; bind() treats
        // kw-only as named params past npos.
        let mut names: Vec<String> = p.pos.iter().map(|x| x.0.clone()).collect();
        names.extend(p.kwonly.iter().map(|x| x.0.clone()));
        let npos = p.pos.len();
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
        let mut fr = Func { vars: HashMap::new(), frames: vec![], class_expr, self_name, exc_vars: vec![], is_module: false, is_class: false, is_async, is_agen: is_async && is_gen, cells: format!("k{id}_") };
        for (n, r) in &captures {
            fr.vars.insert(n.clone(), Var::Cell(r.clone()));
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
        let qname = lit(name);
        let sig = format!("Sig {{ name: {qname}, params: &[{names_lit}], npos: {npos}, star: {star}, kwstar: {kwstar} }}");
        let dflt_all: Vec<String> = dflt_codes.iter().chain(kwd.iter()).cloned().collect();
        let nparams = params.len();
        let unpack: String = (0..nparams).map(|k| format!("__b.next().unwrap()")).collect::<Vec<_>>().join(", ");
        if captures.is_empty() && (at_top || in_class_top) {
            // a plain Rust function
            if is_async {
                let mk = if is_gen { "make_agen" } else { "make_acoro" };
                write!(self.items, "fn {fname}({}) -> R {{ Ok({mk}(sym({qname}), move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}\n", pdecl.join(", ")).unwrap();
            } else if is_gen {
                write!(self.items, "fn {fname}({}) -> R {{ Ok(make_gen(move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}\n", pdecl.join(", ")).unwrap();
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
                return Ok(format!("{{ {sets} func({qname}, move |a: Args| {{ let __d = [{}]; let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {fname}({unpack}) }}) }}", dv.join(", ")));
            }
            let _ = need_defaults;
            return Ok(format!("{{ let __d = vec![{}]; func({qname}, move |a: Args| {{ let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {fname}({unpack}) }}) }}", dflt_all.join(", ")));
        }
        // a closure
        let caps: String = captures.iter().map(|(_, r)| format!("let {r} = {r}.clone(); ")).collect();
        let bind_params: String = params.iter().map(|n| format!("let mut v_{} = __b.next().unwrap(); ", rust_ident(n))).collect();
        if is_gen || is_async {
            let mk = if is_async && is_gen { format!("make_agen(sym({qname}), ") } else if is_async { format!("make_acoro(sym({qname}), ") } else { "make_gen(".to_string() };
            return Ok(format!(
                "{{ let __d = vec![{}]; {caps} func({qname}, move |a: Args| -> R {{ let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {bind_params}{caps} Ok({mk}move || -> R {{ let __depth = Depth::new()?; {decls}{cells_init}\n{body_code}\nOk(V::None) }})) }}) }}",
                dflt_all.join(", ")
            ));
        }
        Ok(format!(
            "{{ let __d = vec![{}]; {caps} func({qname}, move |a: Args| -> R {{ let __depth = Depth::new()?; let mut __b = bind(&{sig}, &__d, a)?.into_iter(); {bind_params}{decls}{cells_init}\n{body_code}\nOk(V::None) }}) }}",
            dflt_all.join(", ")
        ))
    }
    fn class(&mut self, c: &ClassDef) -> ER<String> {
        let id = self.id();
        let mut bases = vec![];
        for b in &c.bases {
            bases.push(self.ex(b, c.line)?);
        }
        // the class body runs with its own namespace: class-level names are Rust locals here,
        // invisible to the methods
        let sc = scope_of(&c.body, &[]);
        let parent = self.fstack.last().unwrap();
        let mut fr = Func { vars: HashMap::new(), frames: parent.frames.clone(), class_expr: parent.class_expr.clone(), self_name: parent.self_name.clone(), exc_vars: parent.exc_vars.clone(), is_module: false, is_class: true, is_async: false, is_agen: false, cells: String::new() };
        let mut decls = String::new();
        for n in sc.locals.iter() {
            let r = format!("a{id}_{}", rust_ident(n));
            write!(decls, "let mut {r}: V = V::Unbound; ").unwrap();
            fr.vars.insert(n.clone(), Var::Local(r));
        }
        let mut fields = vec![];
        for s in &c.body {
            if let St::AnnAssign(Ex::Name(n), _, ann) = &s.st {
                // dataclasses: a ClassVar is no field; InitVar and KW_ONLY are marked
                match ann_kind(ann) {
                    Some("ClassVar") => {}
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
            match self.stmt(s) {
                Ok(x) => {
                    body.push_str(&x);
                    body.push('\n');
                }
                Err(e) => {
                    res = Err(e);
                    break;
                }
            }
        }
        self.fstack.pop();
        self.class_ctx = saved;
        res?;
        // class attributes in the order the body first binds them, as CPython's class namespace keeps them
        // (Enum members and vars() follow it)
        let mut order: Vec<&String> = vec![];
        for s in &c.body {
            let names: Vec<&String> = match &s.st {
                St::Assign(ts, _) => ts.iter().filter_map(|t| if let Ex::Name(n) = t { Some(n) } else { None }).collect(),
                St::AnnAssign(Ex::Name(n), _, _) => vec![n],
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
        for n in order {
            let r = format!("a{id}_{}", rust_ident(n));
            write!(attrs, "if !matches!({r}, V::Unbound) {{ __at.push(({}, {r})); }} ", lit(n)).unwrap();
        }
        // every class has its own __doc__ (None without a docstring), not its parent's
        if !sc.locals.contains("__doc__") {
            let doc = docstring(&c.body).map_or("V::None".to_string(), |d| format!("s({})", lit(d)));
            write!(attrs, "__at.push((\"__doc__\", {doc})); ").unwrap();
        }
        let mk = self.q(&format!("make_class_f({}, {}, vec![{}], __at, vec![{}])", lit(&c.name), lit(if self.module == "__main__" { "__main__" } else { &self.module }), bases.join(", "), fields.join(", ")));
        let mut v = format!("{{ {decls}\n{body}\nlet mut __at: Vec<(Sym, V)> = vec![]; {attrs} let __c = {mk}; __c }}");
        for dec in c.decorators.iter().rev() {
            let dd = self.ex(dec, c.line)?;
            v = self.q(&format!("call(&{dd}, Args::p(vec![{v}]))"));
        }
        Ok(v)
    }
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

// The docstring of a def or class body.
fn docstring(body: &[Stmt]) -> Option<&str> {
    match body.first().map(|s| &s.st) {
        Some(St::Expr(Ex::Str(d))) => Some(d.as_str()),
        _ => None,
    }
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
}
pub fn ic_statics() -> String {
    let n = IC_COUNT.with(|c| c.get());
    let mut s = String::new();
    for k in 0..n {
        s.push_str(&format!("static IC{k}: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);\n"));
    }
    s
}
pub fn program(mods: &[Module], bridge: Bridge) -> Result<(String, Vec<String>), (String, usize, String)> {
    let mut em = Em {
        guards: HashSet::new(),
        out: String::new(),
        items: String::new(),
        n: 0,
        globals: HashMap::new(),
        nglobals: 0,
        consts: vec![],
        const_idx: HashMap::new(),
        fstack: vec![],
        module: String::new(),
        user_modules: mods.iter().skip(1).map(|m| m.name.clone()).collect(),
        direct: HashMap::new(),
        mod_alias: HashMap::new(),
        mod_names: HashMap::new(),
        class_ctx: None,
        next_async: false,
        direct_ok: HashSet::new(),
        bridge,
        bridged_used: BTreeSet::new(),
    };
    // module-level names, import aliases and directly callable defs
    for m in mods {
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
    let mut inits = String::new();
    for m in mods {
        em.module = m.name.clone();
        IN_COPY.with(|c| c.set(OWN_COPIES.lock().unwrap().contains(&m.name)));
        let mut fr = Func { vars: HashMap::new(), frames: vec![], class_expr: None, self_name: None, exc_vars: vec![], is_module: true, is_class: false, is_async: false, is_agen: false, cells: String::new() };
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
        write!(inits, "thread_local! {{ static DONE_{ident}: Cell<bool> = Cell::new(false); }}\nfn init_{ident}() -> Result<(), E> {{ if DONE_{ident}.with(|d| d.replace(true)) {{ return Ok(()); }} gs({name_slot}, s({nm}));\n{body}\nOk(()) }}\n").unwrap();
    }
    let mut out = String::new();
    out.push_str(&em.items);
    out.push_str(&inits);
    // user module attribute hook
    let mut hook = String::from("fn user_mod(m: &str, n: &str) -> Option<R> { match m { ");
    for m in mods.iter().skip(1) {
        write!(hook, "{} => {{ match n {{ ", lit(&m.name)).unwrap();
        for nm in em.mod_names[&m.name].clone() {
            let i = em.gslot(&m.name, &nm);
            write!(hook, "{} => Some(gl({i}, {})), ", lit(&nm), lit(&nm)).unwrap();
        }
        write!(hook, "_ => Some(Err(exc(\"AttributeError\", format!(\"module '{{}}' has no attribute '{{}}'\", m, n)))) }} }} ").unwrap();
    }
    hook.push_str("_ => None } }\n");
    out.push_str(&hook);
    // name -> global slot, per module, for globals()
    for m in mods {
        let mut slots: Vec<(String, usize)> = em.globals.iter().filter(|((mm, n), _)| mm == &m.name && !n.starts_with('<')).map(|((_, n), i)| (n.clone(), *i)).collect();
        slots.sort();
        write!(out, "#[allow(dead_code)] fn gtab_{}(k: &str) -> Option<usize> {{ match k {{ ", rust_ident(&m.name)).unwrap();
        for (n, i) in slots {
            write!(out, "{} => Some({i}), ", lit(&n)).unwrap();
        }
        out.push_str("_ => None } }\n");
    }
    let consts: Vec<String> = em.consts.iter().map(|c| format!("s({})", lit(c))).collect();
    write!(
        out,
        "fn __run() -> Result<(), E> {{ unsafe {{ GL = vec![V::Unbound; {}]; K = vec![{}]; }} USER_MOD.with(|u| u.set(Some(user_mod))); init___main__() }}\nfn main() {{ run_main(__run); }}\n",
        em.nglobals.max(1),
        consts.join(", ")
    )
    .unwrap();
    let _ = &em.out;
    Ok((out, em.bridged_used.into_iter().collect()))
}

// fields used by the code above
impl Em {
    #[allow(dead_code)]
    fn _unused(&self) {}
}

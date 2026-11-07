// ---------- _brevi_ort: ONNX Runtime, for Brevi's onnxruntime (src/dynpy/py/onnxruntime.py) ----------
// The engine is ONNX Runtime's own, through its C API: the glue (src/dynpy/ortglue.rs, pasted into a program that
// imports Brevi's onnxruntime) loads libonnxruntime and sets ORT, so a model computes exactly what CPython's
// onnxruntime computes with it. What is here turns Brevi's numpy arrays into the engine's tensors and its results
// (tensors, sequences, maps) back into arrays, lists and dicts as onnxruntime's Python binding does.

pub struct OrtTensor {
    // ONNXTensorElementDataType
    pub elem: i32,
    pub shape: Vec<i64>,
    // the values, little-endian (one byte per bool)
    pub data: Vec<u8>,
    // the values of a string tensor
    pub strs: Vec<String>,
}

pub enum OrtOut {
    Tensor(OrtTensor),
    Seq(Vec<OrtOut>),
    // keys and values tensors
    Map(OrtTensor, OrtTensor),
    // a value of a kind onnxruntime's Python binding turns into what Brevi's numpy lacks (its ONNX type name)
    Bad(String),
}

pub struct OrtArg {
    pub name: String,
    // onnx's name of the type: "tensor(float)", "seq(map(int64,tensor(float)))"
    pub ty: String,
    // each dimension: Some(n) when known, else its symbolic name ("" for none)
    pub shape: Option<Vec<(Option<i64>, String)>>,
}

pub struct OrtMeta {
    pub producer: String,
    pub graph: String,
    pub domain: String,
    pub description: String,
    pub graph_description: String,
    pub version: i64,
    pub custom: Vec<(String, String)>,
}

pub struct OrtSess {
    pub id: i64,
    pub inputs: Vec<OrtArg>,
    pub outputs: Vec<OrtArg>,
    pub overridable: Vec<OrtArg>,
    pub meta: OrtMeta,
}

#[derive(Default)]
pub struct OrtOpts {
    pub graph_opt: i64,
    pub intra: i64,
    pub inter: i64,
    pub exec_mode: i64,
    pub log_severity: i64,
    pub log_verbosity: i64,
    pub logid: String,
    pub mem_pattern: bool,
    pub cpu_arena: bool,
    pub optimized_path: String,
    pub profiling: bool,
    pub profile_prefix: String,
    pub deterministic: bool,
    pub config: Vec<(String, String)>,
}

pub struct OrtOps {
    // the engine's version
    pub load: fn() -> Result<String, String>,
    pub providers: fn() -> Vec<String>,
    pub build_info: fn() -> String,
    // a session from a model file (Ok) or the model's bytes; errors are (OrtErrorCode, message)
    pub session: fn(Result<&str, &[u8]>, &OrtOpts) -> Result<OrtSess, (i64, String)>,
    // run(session, output names, inputs, (log severity, verbosity, tag, terminate))
    pub run: fn(i64, &[String], &[(String, OrtTensor)], (i64, i64, &str, bool)) -> Result<Vec<OrtOut>, (i64, String)>,
    pub free: fn(i64),
}

pub static ORT: std::sync::OnceLock<OrtOps> = std::sync::OnceLock::new();

fn ort_ops() -> Result<&'static OrtOps, E> {
    ORT.get().ok_or_else(|| exc("OSError", "brevi: this program was built without ONNX Runtime".into()))
}

// onnxruntime's Python errors: the copy raises the class named by the code with the message
fn ort_err((code, m): (i64, String)) -> E {
    exc_v("RuntimeError", tup(vec![V::Int(code), st(m)]))
}

fn ort_s(v: &V) -> String {
    match v {
        V::Str(x) => x.s.clone(),
        _ => String::new(),
    }
}

fn ort_i(v: &V) -> i64 {
    match v {
        V::Int(i) => *i,
        V::Bool(b) => *b as i64,
        _ => 0,
    }
}

fn ort_args(a: Vec<OrtArg>) -> V {
    list(
        a.into_iter()
            .map(|x| {
                let shape = match x.shape {
                    None => V::None,
                    Some(d) => list(
                        d.into_iter()
                            .map(|(n, sym)| match n {
                                Some(n) => V::Int(n),
                                None if !sym.is_empty() => st(sym),
                                None => V::None,
                            })
                            .collect(),
                    ),
                };
                tup(vec![st(x.name), st(x.ty), shape])
            })
            .collect(),
    )
}

// the numpy dtype name of an element type, for errors
fn ort_elem_name(e: i32) -> &'static str {
    match e {
        1 => "float",
        2 => "uint8",
        3 => "int8",
        4 => "uint16",
        5 => "int16",
        6 => "int32",
        7 => "int64",
        8 => "string",
        9 => "bool",
        10 => "float16",
        11 => "double",
        12 => "uint32",
        13 => "uint64",
        14 => "complex64",
        15 => "complex128",
        16 => "bfloat16",
        17 => "float8e4m3fn",
        18 => "float8e4m3fnuz",
        19 => "float8e5m2",
        20 => "float8e5m2fnuz",
        21 => "uint4",
        22 => "int4",
        23 => "float4e2m1",
        _ => "undefined",
    }
}

// A tensor the engine gave as an array of Brevi's numpy.
fn ort_array(t: OrtTensor) -> R {
    let shape: Vec<usize> = t.shape.iter().map(|&d| d.max(0) as usize).collect();
    let d = &t.data;
    let b = match t.elem {
        1 => Buf::F32(d.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()),
        11 => Buf::F(d.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()),
        7 => Buf::I(d.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect()),
        6 => Buf::I32(d.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()),
        9 => Buf::B(d.iter().map(|&c| c != 0).collect()),
        8 => return Err(np_unsup(&format!("a tensor(string) result (numpy's object arrays)"))),
        e => return Err(np_unsup(&format!("a tensor({}) result (numpy {})", ort_elem_name(e), ort_np_name(e)))),
    };
    let ll = t.elem == 7;
    let a = NdArr::new(b, shape);
    // onnxruntime's binding makes int64 results numpy.longlong arrays
    if ll {
        np_mark_ll(&a);
    }
    Ok(np_arr(a))
}

fn ort_np_name(e: i32) -> &'static str {
    match e {
        2 => "uint8",
        3 => "int8",
        4 => "uint16",
        5 => "int16",
        10 => "float16",
        12 => "uint32",
        13 => "uint64",
        14 => "complex64",
        15 => "complex128",
        _ => "dtypes",
    }
}

// The scalars of a map's keys or values, as Python values (float32 values become Python floats).
fn ort_scalars(t: &OrtTensor) -> Result<Vec<V>, E> {
    let d = &t.data;
    Ok(match t.elem {
        1 => d.chunks_exact(4).map(|c| V::Float(f32::from_le_bytes(c.try_into().unwrap()) as f64)).collect(),
        11 => d.chunks_exact(8).map(|c| V::Float(f64::from_le_bytes(c.try_into().unwrap()))).collect(),
        7 => d.chunks_exact(8).map(|c| V::Int(i64::from_le_bytes(c.try_into().unwrap()))).collect(),
        6 => d.chunks_exact(4).map(|c| V::Int(i32::from_le_bytes(c.try_into().unwrap()) as i64)).collect(),
        8 => t.strs.iter().map(|x| st(x.clone())).collect(),
        e => return Err(np_unsup(&format!("a map of {}", ort_elem_name(e)))),
    })
}

fn ort_py(o: OrtOut) -> R {
    match o {
        OrtOut::Tensor(t) => ort_array(t),
        OrtOut::Seq(v) => Ok(list(v.into_iter().map(ort_py).collect::<Result<_, _>>()?)),
        OrtOut::Map(k, v) => {
            let mut d = Dict::new();
            for (k, v) in ort_scalars(&k)?.into_iter().zip(ort_scalars(&v)?) {
                d.set(k, v)?;
            }
            Ok(V::Dict(Rc::new(RefCell::new(d))))
        }
        OrtOut::Bad(t) => Err(np_unsup(&format!("a {t} result"))),
    }
}

// An array of Brevi's numpy as a tensor for the engine.
fn ort_tensor(v: &V) -> Result<OrtTensor, E> {
    let Some(a) = as_np(v) else { return Err(exc("TypeError", format!("brevi: Brevi's onnxruntime takes numpy arrays, not {}", tn(v)))) };
    let shape = a.shape.iter().map(|&d| d as i64).collect();
    let (elem, data, strs) = match a.vals() {
        Buf::F32(x) => (1, x.iter().flat_map(|y| y.to_le_bytes()).collect(), vec![]),
        Buf::F(x) => (11, x.iter().flat_map(|y| y.to_le_bytes()).collect(), vec![]),
        Buf::I(x) => (7, x.iter().flat_map(|y| y.to_le_bytes()).collect(), vec![]),
        Buf::I32(x) => (6, x.iter().flat_map(|y| y.to_le_bytes()).collect(), vec![]),
        Buf::B(x) => (9, x.iter().map(|&y| y as u8).collect(), vec![]),
        Buf::U(x, _) => (8, vec![], x.iter().map(|&i| np_sget(i).to_string()).collect()),
    };
    Ok(OrtTensor { elem, shape, data, strs })
}

pub fn brevi_ort_fn(n: &str, a: Args) -> R {
    let arg = |i: usize| a.p.get(i).cloned().unwrap_or(V::None);
    let o = ort_ops()?;
    match n {
        "load" => (o.load)().map(st).map_err(|m| exc("ImportError", m)),
        "providers" => Ok(list((o.providers)().into_iter().map(st).collect())),
        "build_info" => Ok(st((o.build_info)())),
        // session(path or bytes, options as [(name, value)]) -> (id, inputs, outputs, overridable, meta)
        "session" => {
            let mut opts = OrtOpts { graph_opt: 99, log_severity: -1, mem_pattern: true, cpu_arena: true, profile_prefix: "onnxruntime_profile_".into(), ..Default::default() };
            for p in to_vec(&arg(1))? {
                let kv = to_vec(&p)?;
                let v = kv[1].clone();
                match ort_s(&kv[0]).as_str() {
                    "graph_optimization_level" => opts.graph_opt = ort_i(&v),
                    "intra_op_num_threads" => opts.intra = ort_i(&v),
                    "inter_op_num_threads" => opts.inter = ort_i(&v),
                    "execution_mode" => opts.exec_mode = ort_i(&v),
                    "log_severity_level" => opts.log_severity = ort_i(&v),
                    "log_verbosity_level" => opts.log_verbosity = ort_i(&v),
                    "logid" => opts.logid = ort_s(&v),
                    "enable_mem_pattern" => opts.mem_pattern = truth(&v)?,
                    "enable_cpu_mem_arena" => opts.cpu_arena = truth(&v)?,
                    "optimized_model_filepath" => opts.optimized_path = ort_s(&v),
                    "enable_profiling" => opts.profiling = truth(&v)?,
                    "profile_file_prefix" => opts.profile_prefix = ort_s(&v),
                    "use_deterministic_compute" => opts.deterministic = truth(&v)?,
                    "config" => {
                        for c in to_vec(&v)? {
                            let c = to_vec(&c)?;
                            opts.config.push((ort_s(&c[0]), ort_s(&c[1])));
                        }
                    }
                    _ => {}
                }
            }
            let m = arg(0);
            let bytes = bytes_of(&m);
            let path = ort_s(&m);
            let src = match &bytes {
                Some(b) => Err(b.as_slice()),
                None => Ok(path.as_str()),
            };
            let s = (o.session)(src, &opts).map_err(ort_err)?;
            let mut custom = Dict::new();
            for (k, v) in s.meta.custom {
                custom.set(st(k), st(v))?;
            }
            let meta = tup(vec![
                st(s.meta.producer),
                st(s.meta.graph),
                st(s.meta.domain),
                st(s.meta.description),
                st(s.meta.graph_description),
                V::Int(s.meta.version),
                V::Dict(Rc::new(RefCell::new(custom))),
            ]);
            Ok(tup(vec![V::Int(s.id), ort_args(s.inputs), ort_args(s.outputs), ort_args(s.overridable), meta]))
        }
        // run(id, output names, [(name, array)], (severity, verbosity, tag, terminate)) -> [results]
        "run" => {
            let names: Vec<String> = to_vec(&arg(1))?.iter().map(ort_s).collect();
            let mut ins = vec![];
            for p in to_vec(&arg(2))? {
                let kv = to_vec(&p)?;
                ins.push((ort_s(&kv[0]), ort_tensor(&kv[1])?));
            }
            let ro = to_vec(&arg(3))?;
            let tag = ort_s(&ro[2]);
            let outs = (o.run)(ort_i(&arg(0)), &names, &ins, (ort_i(&ro[0]), ort_i(&ro[1]), &tag, truth(&ro[3])?)).map_err(ort_err)?;
            Ok(list(outs.into_iter().map(ort_py).collect::<Result<_, _>>()?))
        }
        "free" => {
            (o.free)(ort_i(&arg(0)));
            Ok(V::None)
        }
        _ => Err(unsupported(&format!("_brevi_ort.{n}"))),
    }
}

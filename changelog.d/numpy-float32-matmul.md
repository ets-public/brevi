# numpy float32 @ / dot / matmul natively, bit for bit with numpy's OpenBLAS (SkylakeX/AVX-512)

`np_dot2_f32` reproduces OpenBLAS 0.3.34's SkylakeX float32 kernels exactly, on AVX-512 CPUs only
(the same gate as the float32 ufuncs in emit.rs): sdot (sdot_microk_skylakex-2.c, unit and strided,
including its double-accumulated tail), sgemm (the 16x4 kernel through level3.c's GEMM_Q=448 k-blocking),
sgemv_t and sgemv_n (the haswell/skylakex microkernels and their drivers), and the sgemv_t SkylakeX
small kernel (k <= 8). This includes the details that decide the last bit: the gemv drivers' thread
split of rows and the kind-2/kind-1 kernels the last 1-3 rows of each chunk get, the mul+add (not FMA)
4x2/4x1 accumulators, the FMA the compiler fuses into the driver's m3 tail adds, and the per-block
reduction trees of the small kernels. Off AVX-512 the runtime keeps refusing, so those programs keep
running in CPython as before.

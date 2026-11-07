import numpy as np
from safetensors import safe_open
from safetensors.numpy import load, load_file, save, save_file

# Reading a file needs the read grant and saving one the write grant: under --sandbox the build refuses this
# program without them. save and load on bytes need nothing.
save_file({"a": np.ones(2)}, "/tmp/brevi_sb_st.safetensors")
print(load_file("/tmp/brevi_sb_st.safetensors"))
with safe_open("/tmp/brevi_sb_st.safetensors", framework="np") as f:
    print(f.keys())
print(load(save({"a": np.ones(2)})))

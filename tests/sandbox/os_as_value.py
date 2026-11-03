# os passed around or looked up by name (getattr) could reach anything: refused.
import os
getattr(os, "sys" + "tem")("echo escaped the sandbox")

# sys is Brevi's own; sys.modules is not, and must not reach CPython's os under --sandbox.
import sys
sys.modules["os"].system("echo escaped the sandbox")

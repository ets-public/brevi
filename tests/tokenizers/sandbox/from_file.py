# Brevi reads Tokenizer.from_file's file while it builds the program: under --sandbox that needs the read grant.
from tokenizers import Tokenizer

t = Tokenizer.from_file("tests/sandbox/scoped/secret.txt")
print(t.get_vocab_size())

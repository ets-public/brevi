# What AI code does with tiktoken: count chat tokens (OpenAI cookbook), chunk text for RAG, trim to a budget.
import tiktoken


def num_tokens_from_messages(messages, model="gpt-4o-mini"):
    try:
        encoding = tiktoken.encoding_for_model(model)
    except KeyError:
        print("Warning: model not found. Using o200k_base encoding.")
        encoding = tiktoken.get_encoding("o200k_base")
    tokens_per_message = 3
    tokens_per_name = 1
    num_tokens = 0
    for message in messages:
        num_tokens += tokens_per_message
        for key, value in message.items():
            num_tokens += len(encoding.encode(value))
            if key == "name":
                num_tokens += tokens_per_name
    num_tokens += 3
    return num_tokens


messages = [
    {"role": "system", "content": "You are a helpful, pattern-following assistant that translates corporate jargon into plain English."},
    {"role": "system", "name": "example_user", "content": "New synergies will help drive top-line growth."},
    {"role": "system", "name": "example_assistant", "content": "Things working well together will increase revenue."},
    {"role": "user", "content": "This late pivot means we don't have time to boil the ocean for the client deliverable."},
]
for model in ["gpt-4o-mini", "gpt-4o", "gpt-4", "gpt-3.5-turbo", "unknown-model"]:
    print(model, num_tokens_from_messages(messages, model))


def chunk(text, enc, size, overlap):
    toks = enc.encode(text)
    out = []
    start = 0
    while start < len(toks):
        piece = toks[start:start + size]
        out.append(enc.decode(piece))
        if start + size >= len(toks):
            break
        start += size - overlap
    return out


doc = " ".join(
    f"Section {i}: The quick brown fox jumps over the lazy dog {i * 7} times, and résumé #{i} is ready."
    for i in range(60)
)
enc = tiktoken.get_encoding("cl100k_base")
chunks = chunk(doc, enc, 64, 8)
print(len(enc.encode(doc)), len(chunks))
for c in chunks[:3]:
    print(len(enc.encode(c)), repr(c[:70]))
print(sum(len(enc.encode(c)) for c in chunks))


def truncate(text, enc, budget):
    toks = enc.encode(text)
    return text if len(toks) <= budget else enc.decode(toks[:budget])


print(repr(truncate(doc, enc, 20)))
print(repr(truncate("short", enc, 20)))

counts = {}
for word in doc.split()[:200]:
    n = len(enc.encode(" " + word))
    counts[n] = counts.get(n, 0) + 1
print(sorted(counts.items()))

import sys

lines = sys.stdin.read().split('\n')
W = int(lines[0].strip())

paragraphs = []
cur_words = []
for line in lines[1:]:
    if line.strip() == '':
        if cur_words:
            paragraphs.append(cur_words)
            cur_words = []
    else:
        cur_words.extend(line.split())
if cur_words:
    paragraphs.append(cur_words)

outs = []
for words in paragraphs:
    wrapped = []
    cur = ''
    for w in words:
        if cur == '':
            cur = w
        elif len(cur) + 1 + len(w) <= W:
            cur += ' ' + w
        else:
            wrapped.append(cur)
            cur = w
    if cur:
        wrapped.append(cur)
    outs.append('\n'.join(wrapped))

print('\n\n'.join(outs))

import sys,re
for l in sys.stdin:
    l=l.rstrip('\n')
    if not l: continue
    c,_,t=l.partition(' ')
    if c=='e':
        o=[];i=0
        while i<len(t):
            j=i
            while j<len(t) and t[j]==t[i]: j+=1
            o.append(t[i]+str(j-i));i=j
        print(''.join(o))
    else:
        print(''.join(ch*int(n) for ch,n in re.findall(r'(\D)(\d+)',t)))

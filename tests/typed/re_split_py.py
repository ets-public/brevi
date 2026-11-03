import re
def go(tpl):
    parts = re.split(r'(\{\{.*?\}\})', tpl, flags=re.S)
    print(parts)
    print(re.split(r'\{\{.*?\}\}', tpl, flags=re.S))
    print(re.split(r'(a)|(b)', 'xaybz'))
go("Hi {{ name\n }}, you owe {{amt}}.")
def more(s):
    print(re.split(r'x*', s), re.split(r'\b', 'a b'), re.split(r',', ',a,,b,'), re.split(r'(,)', 'a,b'))
more("axbc")

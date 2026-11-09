import yaml

data = yaml.safe_load("a: 1\nb: [2, 3]\nc: hello\n")
print(data)
print(data["a"], data["c"])
print(data["b"][1] + 40)

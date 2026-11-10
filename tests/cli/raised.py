class MyErr(Exception):
    pass
print("before")
raise MyErr("custom failure")

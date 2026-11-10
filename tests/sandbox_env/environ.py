# Under --sandbox the program starts with a short environment: a variable of the caller's is not in
# /proc/self/environ, even with the read grant (check.sh sets SECRET_TEST_TOKEN for this run).
print("SECRET_TEST_TOKEN" in open("/proc/self/environ").read())

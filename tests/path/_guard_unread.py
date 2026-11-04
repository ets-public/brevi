def f():
    try:
        raise ExceptionGroup("g", [ValueError(1)])
    except* ValueError:
        pass
    print({3, -1, 5})

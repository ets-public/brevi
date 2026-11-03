# A list name set to None by a nested def (nonlocal): `is None` sees it.
def main():
    rounds = 0
    players = None
    def finish():
        nonlocal rounds, players
        rounds += 1
        print("round", rounds, players)
        players = None
    for x in ["a", "b", "", "c", ""]:
        if not x:
            if players is not None:
                finish()
            continue
        if players is None:
            players = []
        players.append(x)
    if players is not None:
        finish()
    print(rounds, players is None)
main()

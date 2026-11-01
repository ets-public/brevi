import tkinter as tk

SIZE = 300
CELL = SIZE // 3
LINES = [(0, 1, 2), (3, 4, 5), (6, 7, 8), (0, 3, 6), (1, 4, 7), (2, 5, 8), (0, 4, 8), (2, 4, 6)]


class Game:
    def __init__(self, root):
        self.root = root
        root.title("Tic-tac-toe")
        self.status = tk.Label(root, text="")
        self.status.pack()
        self.canvas = tk.Canvas(root, width=SIZE, height=SIZE)
        self.canvas.pack()
        self.canvas.bind("<Button-1>", self.tap)
        bar = tk.Frame(root)
        bar.pack()
        tk.Button(bar, text="New game", command=self.new_game).pack(side="left")
        self.score = tk.Label(bar, text="")
        self.score.pack(side="left")
        self.wins = {"X": 0, "O": 0, "draw": 0}
        self.timer = None
        self.first = "X"
        self.new_game()

    def new_game(self):
        if self.timer is not None:
            self.root.after_cancel(self.timer)
            self.timer = None
        self.cells = [""] * 9
        self.turn = self.first
        self.first = "O" if self.first == "X" else "X"
        self.over = False
        self.canvas.delete("all")
        for i in (1, 2):
            self.canvas.create_line(i * CELL, 0, i * CELL, SIZE, width=2)
            self.canvas.create_line(0, i * CELL, SIZE, i * CELL, width=2)
        self.status.config(text=f"{self.turn} to move")
        self.show_score()

    def show_score(self):
        self.score.config(text=f"X {self.wins['X']} - O {self.wins['O']} - draws {self.wins['draw']}")

    def tap(self, e):
        if self.over:
            return
        if not (0 <= e.x < SIZE and 0 <= e.y < SIZE):
            return
        i = (e.y // CELL) * 3 + e.x // CELL
        if self.cells[i]:
            self.status.config(text=f"Taken, {self.turn} to move")
            return
        self.cells[i] = self.turn
        cx = (i % 3) * CELL + CELL // 2
        cy = (i // 3) * CELL + CELL // 2
        color = "blue" if self.turn == "X" else "red"
        self.canvas.create_text(cx, cy, text=self.turn, fill=color)
        for a, b, c in LINES:
            if self.cells[a] and self.cells[a] == self.cells[b] == self.cells[c]:
                self.finish(f"{self.turn} wins!", self.turn)
                ax, ay = (a % 3) * CELL + CELL // 2, (a // 3) * CELL + CELL // 2
                bx, by = (c % 3) * CELL + CELL // 2, (c // 3) * CELL + CELL // 2
                self.canvas.create_line(ax, ay, bx, by, fill="green", width=4)
                return
        if all(self.cells):
            self.finish("Draw!", "draw")
            return
        self.turn = "O" if self.turn == "X" else "X"
        self.status.config(text=f"{self.turn} to move")

    def finish(self, msg, who):
        self.over = True
        self.wins[who] += 1
        self.show_score()
        self.msg = msg
        self.countdown(3)

    def countdown(self, n):
        if n == 0:
            self.timer = None
            self.new_game()
            return
        self.status.config(text=f"{self.msg} New game in {n}")
        self.timer = self.root.after(1000, lambda: self.countdown(n - 1))


def main():
    root = tk.Tk()
    Game(root)
    root.mainloop()


main()

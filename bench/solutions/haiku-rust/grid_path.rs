use std::collections::VecDeque;
use std::io::{self, BufRead};

fn main() {
    let grid: Vec<Vec<char>> = io::stdin()
        .lock()
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().chars().collect())
        .collect();

    let h = grid.len();
    let mut start = None;
    let mut end = None;
    for (r, row) in grid.iter().enumerate() {
        for (c, &ch) in row.iter().enumerate() {
            if ch == 'S' {
                start = Some((r, c));
            } else if ch == 'E' {
                end = Some((r, c));
            }
        }
    }
    let (sr, sc) = match start {
        Some(p) => p,
        None => {
            println!("-1");
            return;
        }
    };
    let (er, ec) = match end {
        Some(p) => p,
        None => {
            println!("-1");
            return;
        }
    };

    let mut dist: Vec<Vec<i64>> = grid.iter().map(|row| vec![-1; row.len()]).collect();
    let mut q = VecDeque::new();
    dist[sr][sc] = 0;
    q.push_back((sr, sc));
    let dirs: [(i64, i64); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    while let Some((r, c)) = q.pop_front() {
        if (r, c) == (er, ec) {
            break;
        }
        for &(dr, dc) in dirs.iter() {
            let nr = r as i64 + dr;
            let nc = c as i64 + dc;
            if nr < 0 || nc < 0 || nr as usize >= h {
                continue;
            }
            let (nr, nc) = (nr as usize, nc as usize);
            if nc >= grid[nr].len() || grid[nr][nc] == '#' || dist[nr][nc] != -1 {
                continue;
            }
            dist[nr][nc] = dist[r][c] + 1;
            q.push_back((nr, nc));
        }
    }
    println!("{}", dist[er][ec]);
}

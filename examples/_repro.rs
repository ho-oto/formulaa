
use mascii::ast::{normalize, strip_spacers, Node, Row};
use mascii::render::{render_row, RenderCtx};
use mascii::parse::parse;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const ATOMS: &[char] = &[
    'a', 'b', 'c', 'x', 'y', 'z', 'A', 'B', 'N', '0', '1', '2', '7', '+',
    '-', '=', '<', 'α', 'β', 'π', 'λ', '∞', '∂', '⋅', '±', '∈', '→', '␣', '~',
];

fn gen_row(rng: &mut Rng, depth: usize, max_len: usize) -> Row {
    let len = rng.below(max_len + 1);
    (0..len).map(|_| gen_node(rng, depth)).collect()
}

fn gen_node(rng: &mut Rng, depth: usize) -> Node {
    let structural = depth > 0 && rng.below(100) < 45;
    if !structural {
        return match rng.below(10) {
            0 => Node::Func(["sin", "cos", "log", "exp"][rng.below(4)].into()),
            1 => Node::Accent {
                accent: ['^', '¯', '˙', '⇀', '˜', '‗'][rng.below(6)],
                base: ['x', 'v', 'a', 'E'][rng.below(4)],
            },
            2 => Node::Spacer,
            _ => Node::Sym(ATOMS[rng.below(ATOMS.len())]),
        };
    }
    let d = depth - 1;
    match rng.below(10) {
        0 => Node::Frac { num: gen_row(rng, d, 3), den: gen_row(rng, d, 3) },
        1 => Node::Sqrt { arg: gen_row(rng, d, 3), index: [2, 2, 3, 4][rng.below(4)] },
        2 => Node::Sup { arg: gen_row(rng, d, 2) },
        3 => Node::Sub { arg: gen_row(rng, d, 2) },
        4 => Node::BigOp {
            op: ['∑', '∏', '∫', '⋃'][rng.below(4)],
            lower: gen_row(rng, d, 3),
            upper: gen_row(rng, d, 2),
        },
        5 => {
            // Random delimiter block: any pair (mismatched allowed), with
            // an occasional │ middle. normalize repairs constraint slips.
            let pairs = [
                ('(', ')'),
                ('[', ']'),
                ('{', '}'),
                ('⟨', '⟩'),
                ('|', '|'),
                ('.', '.'),
                ('(', ']'),
                ('{', '.'),
            ];
            let (l, r) = pairs[rng.below(pairs.len())];
            let nsegs = 1 + rng.below(2); // 1 or 2 segs
            let segs = (0..nsegs).map(|_| gen_row(rng, d, 3)).collect::<Vec<_>>();
            Node::Delim { left: l, right: r, mids: vec!['|'; nsegs - 1], segs }
        }
        7 => Node::Cancel { arg: gen_row(rng, d, 3) },
        6 => {
            // Grid inside a random known pair (bracket matrix most often).
            let pairs = [('[', ']'), ('[', ']'), ('(', ')'), ('.', '.'), ('{', '.')];
            let (l, r) = pairs[rng.below(pairs.len())];
            let (rows, cols) = [(2, 2), (1, 2), (2, 1), (1, 1)][rng.below(4)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Delim {
                left: l,
                right: r,
                mids: vec![],
                segs: vec![vec![Node::Array { rows, cols, cells }]],
            }
        }
        8 => {
            // Stray Array: normalize must wrap it in the null delimiter.
            let (rows, cols) = [(2, 2), (1, 2)][rng.below(2)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Array { rows, cols, cells }
        }
        _ => Node::Sym(ATOMS[rng.below(ATOMS.len())]),
    }
}


fn main() {
    let mut rng = Rng(0x8bad_f00d_dead_beef);
    for i in 0..=1391usize {
        let depth = 1 + rng.below(4);
        let row = gen_row(&mut rng, depth, 5);
        if i == 1391 {
            let n1 = normalize(&row);
            println!("normalized: {:?}", n1);
            let expected = normalize(&strip_spacers(&n1));
            println!("expected:   {:?}", expected);
            let aa = render_row(&n1, None, false, &RenderCtx::canonical()).to_text();
            match parse(&aa) {
                Ok(p) if p == expected => println!("roundtrip OK"),
                Ok(p) => println!("MISMATCH\nparsed: {:?}", p),
                Err(e) => println!("parse error: {}", e),
            }
        }
    }
}

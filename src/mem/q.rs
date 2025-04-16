use sacapart::PartitionedSuffixArray;
use std::fs;

#[derive(Debug)]
struct Copy {
    sidx: usize,
    tidx: usize,
    len: usize
}

#[derive(Debug)]
struct Insert<'a> {
    tidx: usize,
    data: &'a [u8]
}

#[derive(Debug)]
enum DiffCommands<'a> {
    Copy(Copy),
    Insert(Insert<'a>)
}

#[inline]
fn usz(i: isize) -> usize {
    debug_assert!(i >= 0);
    i as usize
}

fn qsufsort(i: &mut [isize], v: &mut [isize], buf: &[u8]) {
    debug_assert!(i.len() == buf.len() + 1 && v.len() == i.len(), "Incorrect input buffer size of sort function");
}

fn search(src: &[u8], trg: &[u8], i: &[isize]) -> (usize, usize) {
    (0, 0)
}

fn diff(a: & [u8], b: & [u8]) {
    let n = a.len();
    let m = b.len();
    let mut I: Vec<isize> = vec![0; n+1];
    let mut V: Vec<isize> = vec![0; n+1];
    qsufsort(&mut I, &mut V, a);

    let mut res: Vec<DiffCommands> = Vec::new();

    let mut i: usize = 0;
    let mut last_idx: usize = 0;
    while i < m {
        let (idx, len) = search(a, &b[i..], &I[..]);
        if len <= 4 {
            i += 1;
            continue;
        }
        if last_idx != i - 1 {
            res.push(DiffCommands::Insert(Insert {
                tidx: last_idx+1,
                data: &b[last_idx+1..i-1]  
            }));
        }
        res.push(DiffCommands::Copy(Copy { 
            sidx: idx, 
            tidx: i, 
            len 
        }));

        last_idx = i;
        i += len;
    }
    if last_idx != i-1 {
        res.push(DiffCommands::Insert(Insert {
            tidx: last_idx+1,
            data: &b[last_idx+1..i-1]  
        }));
    }

    for e in res {
        match e {
            DiffCommands::Copy(c) => {
                print!("Copy si: {}, ti: {}, len: {}\n", c.sidx, c.tidx, c.len);
            }
            DiffCommands::Insert(i) => {
                print!("Insert ti: {}, data: {:?}", i.tidx, i.data);
            }
        }
    }
}

fn main() {
    let a = fs::read("D:\\projects\\MineGitFork\\fileA.txt").unwrap();
    let b = fs::read("D:\\projects\\MineGitFork\\fileB.txt").unwrap();
    print!("Data red\nStarting diff generation:\n");

    diff(&a[..], &b[..]);

    // let diff: Vec<DiffCommands> = Vec::new();
    // print!("End diff: {:?}\n", diff);
}
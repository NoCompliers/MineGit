/*inspiration: https://crates.io/crates/sa-is */

const ALPHABET_SIZE: usize = 257;

fn get_suffix_array(data: &[u8]) -> Vec<usize> {
    if data.len() == 0 { return vec![]; }
    if data.len() == 1 { return vec![0]; }
    let n = data.len() + 1;
    let d_len = data.len();
    let mut is_s = vec![false; n];
    is_s[n-1] = true;
    is_s[n-2] = false;

    let mut bucket_sizes: Vec<usize> = vec![0; ALPHABET_SIZE];
    bucket_sizes[data[d_len-1] as usize] += 1;
    bucket_sizes[ALPHABET_SIZE-1] = 1;

    for i in (0..d_len-1).rev() {
        is_s[i] = data[i] < data[i+1] || (data[i] == data[i+1] && is_s[i+1]);
        bucket_sizes[data[i] as usize] += 1;
    }

    let mut bucket_top = vec![0usize; ALPHABET_SIZE];
    let mut bucket_end = vec![0usize; ALPHABET_SIZE];
    bucket_top[ALPHABET_SIZE-1] = 0;
    bucket_end[ALPHABET_SIZE-1] = 0;
    let mut idx = 1;
    for i in 0..ALPHABET_SIZE-1 {
        bucket_top[i] = idx;
        idx += bucket_sizes[i];
        
        debug_assert!(idx != 0);
        bucket_end[i] = idx - 1;
    }

    let mut lms: Vec<usize> = Vec::new();
    let mut idxs: Vec<usize> = vec![usize::MAX; n];
    idxs[0] = d_len;

    for i in (1..n-1).rev() {
        if is_s[i] && !is_s[i-1] {
            lms.push(i);

            let idx = bucket_end[data[i] as usize];
            idxs[idx] = i;
            bucket_end[data[i] as usize] -= 1;
        }
    }

    for i in 0..n {
        if idxs[i] != usize::MAX && idxs[i] != 0 && !is_s[idxs[i]-1] {
            let idx = bucket_top[data[idxs[i]-1] as usize];
            idxs[idx] = idxs[i]-1;
            bucket_top[data[idxs[i]-1] as usize] += 1;
        }
    }
    let mut idx = 1;
    for i in 0..ALPHABET_SIZE-1 {
        idx += bucket_sizes[i];
        bucket_end[i] = idx - 1;
    }

    for i in (0..n).rev() {
        if idxs[i] != usize::MAX && idxs[i] != 0 && is_s[idxs[i]-1] {
            let idx = bucket_end[data[idxs[i]-1] as usize];
            idxs[idx] = idxs[i]-1;
            bucket_end[data[idxs[i]-1] as usize] -= 1;
        }
    }

    print!("Idxs: {:?}\n", idxs.iter().map(|&a| if a == usize::MAX {0} else {a}).collect::<Vec<_>>());

    vec![]
}

fn main() {
    let str = "dabracadabrac";
    get_suffix_array(str.as_bytes());
}
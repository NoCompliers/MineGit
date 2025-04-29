use divsufsort::sort_in_place;
use std::fs::File;
use std::io::{self, Read, Write};
use std::time::Instant;
use crate::recover::diff::*;

const MIN_COPY_SIZE: usize = 16;

pub struct DiffGenerator {
    pub data: Vec<u8>,
    closest: Vec<(usize, usize)>,
    // commands: Vec<DiffCommand<'a>>,
    n: usize
}

impl DiffGenerator {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            closest: Vec::new(),
            // commands: Vec::new(),
            n: 0
        }
    }

    pub fn init<R: Read>(&mut self, src: &mut R, trg: &mut R) -> io::Result<()> {
        let start = Instant::now();
        self.data.clear();

        // Read the contents of both readers into self.data
        self.n = src.read_to_end(&mut self.data)?;
        trg.read_to_end(&mut self.data)?;
        print!("DiffGen init finished, time taken: {:?}\n", start.elapsed().as_millis());
        Ok(())
    }

    fn init_closest(&mut self) {
        let data = &self.data;

        let start= Instant::now();
        let mut idxs = vec![0; data.len()];
        print!("alloc for intervals time taken: {:?}\n", start.elapsed().as_millis());
        let start= Instant::now();
        sort_in_place(data, &mut idxs);
        print!("sorting time taken: {:?}\n", start.elapsed().as_millis());
        let idxs: Vec<usize> = idxs.iter().map(|&x| x as usize).collect();
        print!("with conversion taken: {:?}\n", start.elapsed().as_millis());

        let len = data.len();
        let n = self.n;

        let start = Instant::now();
        let mut last_data: usize = 0;
        let mut i = 0;
        while i < len && idxs[i] >= n { i += 1; }
        let closest = &mut self.closest;
        closest.resize(len - self.n, (usize::MAX, usize::MAX));

        for i in i..len {
            let idx = idxs[i];
            if idx < n {
                for j in last_data..i {
                    closest[idxs[j]-n].1 = idx;
                }
                last_data = i+1;
            } else {
                closest[idx-n].0 = idxs[last_data-1];
            }
        }
        print!("closest find time taken: {:?}\n", start.elapsed().as_millis());
    }

    pub fn generate<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        self.init_closest();
        let data = &self.data;
        let n = self.n;
        let closest = &self.closest;

        let len = data.len();
        let m = len - self.n;

        let mut save_from: usize = 0;
        let mut i = 0;

        let start = Instant::now();
        while i < m {
            let (smaller, bigger) = closest[i];
            let mut l1 = 0;
            while smaller + l1 < n && i + l1 < m && data[smaller + l1] == data[i + l1 + n] {
                l1 += 1;
            }
            let mut l2 = 0;
            while bigger + l2 < n && i + l2 < m && data[bigger + l2] == data[i + l2 + n] {
                l2 += 1;
            }

            let (j, l) = if l1 >= l2 { (smaller, l1) } else { (bigger, l2) };
            if l < MIN_COPY_SIZE { 
                i += 1;
                continue; 
            }
            if save_from != i {
                Insert::serialize(&data[save_from+n..i+n], out)?;
            }
            Copy { sidx: j as u64, len: l as u64 }.serialize(out)?;

            i += l;
            save_from = i;
        }
        print!("finding diff: {:?}\n", start.elapsed().as_millis());
        if save_from != m {
            Insert::serialize(&data[save_from+n..m+n], out)?;
        }
        print!("Init closest finished\n");
        Ok(())
    }
}
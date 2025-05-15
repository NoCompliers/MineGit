#[test]
pub fn test() {
    use super::snapshot::SnapshotHeader;
    use std::{
        fs::{File, OpenOptions},
        io::Read,
    };

    let mut f1 = File::open("/home/vr/Desktop/MineGit/test_files/r.0.0.mca")
        .expect("/home/vr/Desktop/MineGit/test_files/r.0.0.mca was not found for testing");
    let mut f2 = File::open("/home/vr/Desktop/MineGit/test_files/r.0.1.mca")
        .expect("/home/vr/Desktop/MineGit/test_files/r.0.1.mca was not found for testing");
    let mut data1 = Vec::new();
    let mut data2 = Vec::new();
    f1.read_to_end(&mut data1).unwrap();
    f2.read_to_end(&mut data2).unwrap();

    let mut pack = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open("/home/vr/Desktop/MineGit/test_files/pkg.pkg")
        .unwrap();

    let snap1 = SnapshotHeader::save_new(&mut pack, &data1).unwrap();
    let snap2 = snap1.update(&mut pack, &data2).unwrap();

    let _data1 = snap1.recover(&mut pack).unwrap();
    let _data2 = snap2.recover(&mut pack).unwrap();
    if data1 != _data1 {
        panic!("Recover test fail because of incorrect snap1 recovery");
    }
    if data2 != _data2 {
        panic!("Recover test fail because of incorrect snap2 recovery");
    }

    let snap3 = snap2.update(&mut pack, &data1).unwrap();
    let snap4 = snap3.update(&mut pack, &data2).unwrap();
    if snap4
        .recover(&mut pack)
        .expect("Error while recovering snap4")
        != data2
    {
        panic!("Incorect snap4 recover");
    }

    let snap3 = snap1
        .update(&mut pack, &data2)
        .expect("Error while updating snap3 in test");
    let data3 = snap3
        .recover(&mut pack)
        .expect("Error while recovering snap3 in test");
    if data3 != data2 {
        panic!("Recover test fail because of incorrect snap3 recovery");
    }
}

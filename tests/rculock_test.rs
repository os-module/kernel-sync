extern crate alloc;
use alloc::vec;
use kernel_sync::RcuLock;

#[test]
fn basic_test() {
    let x = RcuLock::new(0);
    let thread_cnt = 3;
    let loop_cnt = 1000000;
    // let loop_cnt = 100;
    let mut threads = vec![];
    for _ in 0..thread_cnt {
        let x_clone = x.clone();
        threads.push(std::thread::spawn(move || {
            // let mut guard = x_clone.write();
            for _ in 0..loop_cnt {
                let mut guard = x_clone.write();
                *guard += 1;
            }
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(*(x.read()), thread_cnt * loop_cnt);
}

#[test]
fn try_lock_test() {
    let x = RcuLock::new(0);
    let lock_result0 = x.try_write();
    assert!(lock_result0.is_some());

    let lock_result1 = x.try_write();
    assert!(lock_result1.is_none());

    drop(lock_result0);

    let lock_result2 = x.try_write();
    assert!(lock_result2.is_some());
}

#[test]
fn read_write_test() {
    let x = RcuLock::new(0);
    let thread_cnt = 4;
    // let loop_cnt = 1000000;
    // let loop_cnt = 100;
    let mut threads = vec![];
    for i in 0..thread_cnt {
        let x_clone = x.clone();
        threads.push(std::thread::spawn(move || {
            match i {
                0 => {
                    std::println!("thread0 starts.");
                    let x = x_clone.read();
                    let read_0 = &*x;
                    assert_eq!(*read_0, 0);
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    assert_eq!(*read_0, 0);
                    drop(x);
                    std::println!("thread0 ends.");
                },
                1 => {
                    std::println!("thread1 starts.");
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let mut x = x_clone.write();
                    let write_1 = &mut *x;
                    *write_1 = 1;
                    assert_eq!(*write_1, 1);
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    assert_eq!(*write_1, 1);
                    // std::thread::sleep(std::time::Duration::from_secs(4));
                    drop(x);
                    std::println!("thread1 ends.");
                },
                2 => {
                    std::println!("thread2 starts.");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let x1 = x_clone.read();
                    let read_2 = &*x1;
                    std::println!("read_2 = {read_2}");
                    std::thread::sleep(std::time::Duration::from_secs(7));
                    let x2 = x_clone.read();
                    let read_3 = &*x2;
                    std::println!("read_3 = {read_3}");
                    drop(x1);
                    drop(x2);
                    std::println!("thread2 ends.");
                },
                3 => {
                    std::println!("thread3 starts.");
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    let mut x = x_clone.write();
                    let write_4 = &mut *x;
                    *write_4 += 1;
                    assert_eq!(*write_4, 2);
                    // std::thread::sleep(std::time::Duration::from_secs(6));
                    drop(x);
                    std::println!("thread3 ends.");
                },
                _ => {},
            }
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
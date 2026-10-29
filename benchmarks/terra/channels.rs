use std::sync::mpsc::sync_channel;
use std::thread;

fn main() {
    let (sender, receiver) = sync_channel::<i64>(64);
    let producer = thread::spawn(move || {
        let mut sent = 0;
        for i in 0..100_000 {
            sender.send(i).unwrap();
            sent += 1;
        }
        sent
    });
    let mut count = 0;
    let mut sum = 0i64;
    for _ in 0..100_000 {
        sum += receiver.recv().unwrap();
        count += 1;
    }
    let sent = producer.join().unwrap();
    println!("sent={sent} count={count} sum={sum}");
}

use anyhow::anyhow;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const NUM_PRODUCERS: usize = 4;

#[allow(dead_code)]
#[derive(Debug)]
struct Msg {
    idx: usize,
    value: usize,
}

fn main() -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();

    for i in 0..NUM_PRODUCERS {
        let tx = tx.clone(); // 下面使用的全是 clone 出来的tx，原始的 tx 没人用
        thread::spawn(move || produce(i, tx));
    }

    let consumer = thread::spawn(move || {
        for msg in rx {
            println!("consumer: {:?}", msg);
        }
        println!("consumer exit.");
        42
    });
    drop(tx); // 释放原始的 tx， 否则 rx 无法结束

    let secret = consumer
        .join()
        .map_err(|e| anyhow!("Thread join error: {:?}", e))?;
    println!("secret: {secret}");
    Ok(())
}

fn produce(idx: usize, tx: mpsc::Sender<Msg>) -> anyhow::Result<()> {
    loop {
        let value = rand::random::<usize>();
        tx.send(Msg::new(idx, value))?;
        let sleep_time = rand::random::<u8>() as u64 * 10;
        thread::sleep(Duration::from_millis(sleep_time));

        if rand::random::<u8>() % 10 == 0 {
            // break Ok(());
            break;
        }
    }
    println!("producer {idx} exit.");
    Ok(())
}

impl Msg {
    fn new(idx: usize, value: usize) -> Self {
        Self { idx, value }
    }
}

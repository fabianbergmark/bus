use loom::sync::atomic::AtomicUsize;
use loom::sync::atomic::Ordering;
use loom::thread;

use bus::Bus;

#[derive(Default)]
struct Protocol {
    data: AtomicUsize,
}

#[test]
fn circle() {
    loom::model(|| {
        println!("START ----------------------------------");
        let mut b1: Bus<Protocol> = Bus::new("b1");
        let mut b2: Bus<Protocol> = Bus::new("b2");
        let mut b3: Bus<Protocol> = Bus::new("b3");
        let mut b4: Bus<Protocol> = Bus::new("b4");
        b1.connect(&mut b2);
        b2.connect(&mut b3);
        b3.connect(&mut b4);
        let h1 = thread::spawn(move || {
            for i in 0..1 {
                //set_data(&mut b1, i);
                b1.disconnect(&mut b2);
                //set_data(&mut b2, i);
                b2.connect(&mut b1);
            }
            println!("h1 done");
        });
        let h2 = thread::spawn(move || {
            for i in 0..1 {
                b3.disconnect(&mut b4);
                //b3.disconnect(&mut b4);
                //set_data(&mut b4, i);
                b3.connect(&mut b4);
                //set_data(&mut b4, i);
            }
            println!("h2 done");
        });
        

        h2.join().unwrap();

        h1.join().unwrap();
        println!("END ----------------------------------");
    });
}

fn get_data(bus: &Bus<Protocol>) -> usize {
    bus.get_data().data.load(Ordering::SeqCst)
}

fn set_data(bus: &mut Bus<Protocol>, data: usize) {
    bus.get_data().data.store(data, Ordering::SeqCst);
}

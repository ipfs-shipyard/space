mod plans;
mod utils;

use plans::*;
use std::time::Instant;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    let testing_list: Vec<(&str, fn())> = vec![
        // ("Transmit 5kb Dag", test_transmit_5kb_dag),
        // ("Transmit 500kb Dag", test_transmit_500kb_dag),
        // ("Transmit 5mb Dag", test_transmit_5mb_dag),
        // (
        //     "Transmit 20mb Dag, 5s passes",
        //     test_transmit_20mb_dag_over_5s_passes,
        // ),
        // (
        //     "Transmit 20mb Dag, 5s passes, receiver off outside of pass",
        //     test_transmit_20mb_dag_over_5s_passes_receiver_off,
        // ),
        (
            "Transmit 20mb Dag, 5s passes, transmitter off outside of pass",
            test_transmit_20mb_dag_over_5s_passes_transmitter_off,
        ),
    ];

    for (name, test_fn) in testing_list {
        println!("Running: {name}");
        let start = Instant::now();
        let result = std::panic::catch_unwind(test_fn);
        let end = start.elapsed();
        if result.is_err() {
            println!("Test failed after {end:.2?}\n");
        } else {
            println!("Test passed after {end:.2?}!\n");
        }
    }
}

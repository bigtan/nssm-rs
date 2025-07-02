use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    println!("Test Application Started");
    println!("PID: {}", std::process::id());
    println!("Working Directory: {:?}", std::env::current_dir().unwrap());
    println!("Arguments: {:?}", std::env::args().collect::<Vec<_>>());

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C signal, shutting down gracefully...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let mut counter = 0;
    let args: Vec<String> = std::env::args().collect();

    while running.load(Ordering::SeqCst) {
        counter += 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        println!("[{}] Application heartbeat #{}", timestamp, counter);

        // Handle special arguments for testing
        if args.len() > 1 {
            match args[1].as_str() {
                "error" => {
                    if counter >= 3 {
                        println!("Simulating error exit");
                        std::process::exit(1);
                    }
                }
                "panic" => {
                    if counter >= 5 {
                        panic!("Simulated panic!");
                    }
                }
                "quick" => {
                    if counter >= 2 {
                        println!("Quick exit for throttling test");
                        break;
                    }
                }
                _ => {}
            }
        }

        thread::sleep(Duration::from_secs(2));
    }

    println!("Application shutting down gracefully after {} heartbeats", counter);
}

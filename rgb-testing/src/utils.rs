//! Test Utils
#[macro_export]
macro_rules! wait {
    ($callback:expr, $timeout:expr) => {{
        let mut success = false;
        for wait in 0..$timeout {
            let result = $callback();
            if let Err(_) = result {
                std::thread::sleep(std::time::Duration::from_millis(wait));
                continue;
            }
            log::info!("callback completed in {wait} milliseconds");
            success = true;
            break;
        }
        assert!(success, "callback got a timeout");
    }};
    ($callback:expr) => {
        $crate::wait!($callback, 100);
    };
}

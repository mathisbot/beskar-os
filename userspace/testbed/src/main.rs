#![no_std]
#![no_main]

use beskar_lib::ExitCode;
use testbed::core::TestCase;

beskar_lib::entry_point!(main);

const TESTS: &[TestCase] = &[
    TestCase::new("file-io", testbed::io::file::test_file_io),
    TestCase::new("memory-api", testbed::mem::test_memory_api),
    TestCase::new("sync-api", testbed::sync::test_sync_api),
    TestCase::new("thread-api", testbed::thread::test_thread_api),
    TestCase::new("surface-api", testbed::surface::test_surface_api),
];

fn main(_start: &beskar_lib::ThreadStartBlock) {
    beskar_lib::println!("testbed: running {} userspace API tests", TESTS.len());

    let mut passed = 0usize;
    for test in TESTS {
        beskar_lib::println!("[ RUN ] {}", test.name());
        match test.run() {
            Ok(()) => {
                passed += 1;
                beskar_lib::println!("[ PASS ] {}", test.name());
            }
            Err(msg) => {
                beskar_lib::println!("[ FAIL ] {} ({})", test.name(), msg);
            }
        }
    }

    let failed = TESTS.len() - passed;
    beskar_lib::println!("testbed summary: {} passed, {} failed", passed, failed);

    if failed != 0 {
        beskar_lib::exit(ExitCode::Failure);
    }
}

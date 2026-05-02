use crate::{core::TestResult, ensure};
use beskar_lib::io::{File, Read as _};

pub fn test_file_io() -> TestResult {
    invalid_path()?;
    rand_file()?;

    Ok(())
}

fn invalid_path() -> TestResult {
    ensure!(
        File::open("/invalid/path").is_err(),
        "opening invalid path unexpectedly succeeded",
    );
    Ok(())
}

fn rand_file() -> TestResult {
    let mut rand_file = File::open("/dev/rand").map_err(|_| "open /dev/rand failed")?;
    if rand_file.path() != "/dev/rand" {
        return Err("opened file path does not match expected path");
    }

    let mut buf = [0_u8; 16];
    rand_file
        .read_exact(&mut buf)
        .map_err(|_| "read from /dev/rand failed")?;
    ensure!(
        buf.iter().any(|&byte| byte != 0),
        "/dev/rand returned an all-zero block",
    );
    Ok(())
}

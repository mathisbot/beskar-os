pub type TestResult = Result<(), &'static str>;

pub struct TestCase {
    name: &'static str,
    run: fn() -> TestResult,
}

impl TestCase {
    #[must_use]
    #[inline]
    pub const fn new(name: &'static str, run: fn() -> TestResult) -> Self {
        Self { name, run }
    }

    #[must_use]
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    pub fn run(&self) -> TestResult {
        (self.run)()
    }
}

#[macro_export]
/// Return an error if the condition is not met, with a provided message.
macro_rules! ensure {
    ($cond:expr, $msg:literal $(,)?) => {
        if !$cond {
            return Err($msg);
        }
    };
}

#[macro_export]
///Return an error if the two expressions are not equal, with a provided message.
macro_rules! ensure_eq {
    ($left:expr, $right:expr, $msg:literal $(,)?) => {
        if $left != $right {
            return Err($msg);
        }
    };
}

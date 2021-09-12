pub trait KernelTest {
    fn ktest(&self);
}

#[repr(transparent)]
pub struct TestError(&'static str);

pub enum TestResult {
    Ok,
    Err(TestError),
}

pub(crate) fn test_runner(tests: &[&dyn KernelTest]) {
    kprintln!("Running {} tests", tests.len());
    for test in tests {
        test.ktest();
    }
}

macro_rules! ktest {
    ($(fn $name:ident() -> TestResult $content:block )*) => {
        $(
            ktest!(helper fn $name() -> TestResult $content);
        )*

    };

    (helper fn $name:ident() -> TestResult $content:block ) => {
        #[test_case]
        fn $name() -> TestResult {
            kprint!("running: `{}`", stringify!($name));
            $content
        }
    };
}

impl<F: Fn() -> TestResult> KernelTest for F {
    fn ktest(&self) {
        match self() {
            TestResult::Ok => kprintln!("\t[ok]"),
            TestResult::Err(TestError(msg)) => kprintln!("\t[err]: {}", msg),
        }
    }
}

ktest! {
    fn test_infra_ok() -> TestResult {
        TestResult::Ok
    }

    fn test_infra_err() -> TestResult {
        TestResult::Err(TestError("test infra failed :("))
    }
}

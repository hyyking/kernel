pub trait KernelTest {
    fn ktest(&self) -> TestResult;
}

#[repr(transparent)]
pub struct TestError(&'static str);

pub enum TestResult {
    Ok,
    Err(TestError),
    ShouldFail,
}

pub(crate) fn test_runner(tests: &[&dyn KernelTest]) {
    kprintln!("Running {} tests", tests.len());
    for test in tests {
        match test.ktest() {
            TestResult::Ok => {
                kprintln!("\t[ok]");
            }
            TestResult::Err(TestError(msg)) => {
                kprintln!("\t[err]: {}", msg);
            }
            TestResult::ShouldFail => {
                kprintln!("\t[should_fail]");
            }
        }
    }
}

impl<F: Fn() -> TestResult> KernelTest for F {
    fn ktest(&self) -> TestResult {
        self()
    }
}

#[macro_export]
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

mod tests {
    use super::*;
    use libx64::idt::{lidt, InterruptDescriptorTable as Idt, InterruptFrame};

    static mut IDT: Idt = Idt::new();

    pub extern "x86-interrupt" fn test_int3(_f: InterruptFrame) {
        return;
    }

    ktest! {
        fn test_infra_ok() -> TestResult {
            TestResult::Ok
        }

        fn test_infra_sf() -> TestResult {
            TestResult::ShouldFail
        }

        fn test_infra_err() -> TestResult {
            TestResult::Err(TestError("should fail"))
        }

        fn test_load_idt() -> TestResult {
            unsafe {
                IDT.set_handler(0x03, self::test_int3);
                lidt(&IDT);
            }
            TestResult::Ok
        }

        fn test_call_int3() -> TestResult {
            unsafe {
                asm!("int3");
            }
            TestResult::Ok
        }

    }
}

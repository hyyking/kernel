cfg_qemu! {
    use core::fmt::Write;

    use crate::{
        drivers::serial_uart16550::SerialPort,

    };
    use kcore::sync::{mutex::SpinMutex};

    klazy! {
        // SAFETY: we are the only one accessing this port
        ref static DRIVER: SpinMutex<SerialPort> = unsafe {
            let mut port = SerialPort::new(0x3F8);
            port.init();
            SpinMutex::new(port)
        };
    }

    #[doc(hidden)]
    pub(crate) fn _qprint(args: core::fmt::Arguments) {
        DRIVER.lock().write_fmt(args).expect("qprint");
    }

    #[macro_export]
    macro_rules! qprint {
        ($($arg:tt)*) => ($crate::infra::qemu::_qprint(format_args!($($arg)*)));
    }

    #[macro_export]
    macro_rules! qprintln {
        () => ($crate::qprint!("\n"));
        ($($arg:tt)*) => ($crate::infra::qemu::_qprint(format_args!("{}\n", format_args!($($arg)*))));
    }
}

cfg_not_qemu! {
    #[macro_export]
    macro_rules! qprint {
        ($($arg:tt)*) => ();
    }

    #[macro_export]
    macro_rules! qprintln {
        () => ();
        ($($arg:tt)*) => ();
    }
}

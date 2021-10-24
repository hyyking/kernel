tui enable
file target/target/debug/kernel
target remote :1234
break _start
c

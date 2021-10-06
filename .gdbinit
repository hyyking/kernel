tui enable
file target/target/debug/os
target remote :1234
break _start
c

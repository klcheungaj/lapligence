// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_001/time_units.sv
// IEEE 1800-2009 §§3.14.1-3.14.3 and 20.4: every legal time-unit magnitude
// (1, 10 and 100 of s, ms, us, ns, ps and fs) schedules `#1` exactly one
// local unit after time zero. The finest precision (1fs) is the scheduler
// tick, so 100s is 1e17 ticks; %t reports femtoseconds through $timeformat.
module u_1s;
    timeunit 1s;
    timeprecision 1s;
    initial #1 $display("1s %t", $realtime);
endmodule
module u_10s;
    timeunit 10s;
    timeprecision 10s;
    initial #1 $display("10s %t", $realtime);
endmodule
module u_100s;
    timeunit 100s;
    timeprecision 100s;
    initial #1 $display("100s %t", $realtime);
endmodule
module u_1ms;
    timeunit 1ms;
    timeprecision 1ms;
    initial #1 $display("1ms %t", $realtime);
endmodule
module u_10ms;
    timeunit 10ms;
    timeprecision 10ms;
    initial #1 $display("10ms %t", $realtime);
endmodule
module u_100ms;
    timeunit 100ms;
    timeprecision 100ms;
    initial #1 $display("100ms %t", $realtime);
endmodule
module u_1us;
    timeunit 1us;
    timeprecision 1us;
    initial #1 $display("1us %t", $realtime);
endmodule
module u_10us;
    timeunit 10us;
    timeprecision 10us;
    initial #1 $display("10us %t", $realtime);
endmodule
module u_100us;
    timeunit 100us;
    timeprecision 100us;
    initial #1 $display("100us %t", $realtime);
endmodule
module u_1ns;
    timeunit 1ns;
    timeprecision 1ns;
    initial #1 $display("1ns %t", $realtime);
endmodule
module u_10ns;
    timeunit 10ns;
    timeprecision 10ns;
    initial #1 $display("10ns %t", $realtime);
endmodule
module u_100ns;
    timeunit 100ns;
    timeprecision 100ns;
    initial #1 $display("100ns %t", $realtime);
endmodule
module u_1ps;
    timeunit 1ps;
    timeprecision 1ps;
    initial #1 $display("1ps %t", $realtime);
endmodule
module u_10ps;
    timeunit 10ps;
    timeprecision 10ps;
    initial #1 $display("10ps %t", $realtime);
endmodule
module u_100ps;
    timeunit 100ps;
    timeprecision 100ps;
    initial #1 $display("100ps %t", $realtime);
endmodule
module u_1fs;
    timeunit 1fs;
    timeprecision 1fs;
    initial #1 $display("1fs %t", $realtime);
endmodule
module u_10fs;
    timeunit 10fs;
    timeprecision 10fs;
    initial #1 $display("10fs %t", $realtime);
endmodule
module u_100fs;
    timeunit 100fs;
    timeprecision 100fs;
    initial #1 $display("100fs %t", $realtime);
endmodule

module tb;
    timeunit 1fs;
    timeprecision 1fs;
    u_1s i_1s();
    u_10s i_10s();
    u_100s i_100s();
    u_1ms i_1ms();
    u_10ms i_10ms();
    u_100ms i_100ms();
    u_1us i_1us();
    u_10us i_10us();
    u_100us i_100us();
    u_1ns i_1ns();
    u_10ns i_10ns();
    u_100ns i_100ns();
    u_1ps i_1ps();
    u_10ps i_10ps();
    u_100ps i_100ps();
    u_1fs i_1fs();
    u_10fs i_10fs();
    u_100fs i_100fs();
    initial $timeformat(-15, 0, "", 0);
    initial #200s $finish(0);
endmodule

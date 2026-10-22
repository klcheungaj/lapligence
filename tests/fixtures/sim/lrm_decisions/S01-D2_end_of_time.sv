// Decision S01-D2: simulation time ends at 2^64-1 ticks. A nonblocking
// update whose time would pass it takes effect in the NBA region of the
// current time slot; one that reaches it exactly is scheduled there. A
// blocking delay that would pass it is a run-time error that ends the
// simulation; the final procedures run and the exit status is 1.
//
// IEEE 1800-2009 9.4.1 (SystemVerilog-1800-2009.txt L11749-11750): "If the
// delay expression evaluates to a negative value, it shall be interpreted as
// a twos-complement unsigned integer of the same size as a time variable."
// 3.14 L2807-2809: "The term simulation time is used to refer to the time
// value maintained by the simulator to model the actual time it would take
// for the system description being simulated." The text gives the time no
// range and does not say what happens past the last time value; llg
// follows industry-standard behaviour.
module tb;
  timeunit 1fs;
  timeprecision 1fs;
  logic [7:0] q;
  initial begin
    #(64'hFFFF_FFFF_FFFF_FFF0);
    q <= #20 8'd2;
    q <= #15 8'd1;
    $display("issued at %0d", $time);
    @(q) $display("q=%0d at %0d", q, $time);
    @(q) $display("q=%0d at %0d", q, $time);
    #1 $display("past the end");
  end
  final $display("final q=%0d", q);
endmodule

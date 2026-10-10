// Decision S14-D4: the first event of wait_order may be satisfied by its
// triggered state. An event triggered earlier in the same time step counts
// as the first occurrence, so only the later events need new triggers.
//
// IEEE 1800-2009 15.5.4 (SystemVerilog-1800-2009.txt L20826): "Only the
//   first event in the list can wait for the persistent triggered property."
// 15.5.3 (L20735-20736): "The triggered event property evaluates to true if
//   the given event has been triggered in the current time step and false
//   otherwise."
//
// Expected: `a` is triggered at 1 before wait_order starts in the same step;
// `b` at 2 completes the sequence.
`timescale 1ns / 1ns
module tb;
  event a, b;

  initial begin
    #1 ->a;
    fork
      begin
        wait_order (a, b) $display("%0t success", $time);
        else $display("%0t failure", $time);
      end
      #1 ->b;
    join
    $finish;
  end
endmodule

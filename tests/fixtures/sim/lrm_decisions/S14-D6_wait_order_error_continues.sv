// Decision S14-D6: a failing wait_order without a fail statement reports a
// run-time error (llg: an error message on stderr) and the process continues
// with the statement after it; the simulation is not stopped.
//
// IEEE 1800-2009 15.5.4 (SystemVerilog-1800-2009.txt L20828-20830): "If it
//   is specified, then the given statement is executed upon failure of the
//   construct. If the fail statement is not specified, a failure generates a
//   run-time error."
//
// The text does not say whether the run-time error ends the simulation; llg
// treats it like a `$error` severity message.
// Expected: the failure at 1 (b before a) is reported and both lines print.
`timescale 1ns / 1ns
module tb;
  event a, b;

  initial begin
    fork
      begin
        wait_order (a, b);
        $display("%0t after failure", $time);
      end
      #1 ->b;
    join
    $display("%0t end", $time);
    $finish;
  end
endmodule

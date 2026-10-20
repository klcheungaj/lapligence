// Decision S14-D6: a failing wait_order without a fail statement is a
// run-time error that ends the simulation at once: the waiting process does
// not continue, nothing else runs in that or a later time step, and final
// procedures do not run (llg: an error on stderr, exit status 1).
//
// IEEE 1800-2009 15.5.4 (SystemVerilog-1800-2009.txt L20828-20830): "If it
//   is specified, then the given statement is executed upon failure of the
//   construct. If the fail statement is not specified, a failure generates a
//   run-time error."
//
// The text does not say whether the run-time error ends the simulation; llg
// follows the industry-standard behaviour of ending it.
// Expected: only the line printed before b triggers at 1.
`timescale 1ns / 1ns
module tb;
  event a, b;

  final begin
    $display("final block");
  end

  initial begin
    fork
      begin
        wait_order (a, b);
        $display("%0t after failure", $time);
      end
      #1 begin
        $display("%0t trigger b", $time);
        ->b;
      end
    join
    $display("%0t end", $time);
    $finish;
  end

  initial #2 $display("%0t later process", $time);
endmodule

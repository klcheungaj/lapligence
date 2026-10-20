// SIM-014: a wait_order without a fail statement that fails reports a
// run-time error (SV 15.5.4) that ends the simulation at once (S14-D6).
`timescale 1ns / 1ns
module tb;
  event a, b;
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
  end
endmodule

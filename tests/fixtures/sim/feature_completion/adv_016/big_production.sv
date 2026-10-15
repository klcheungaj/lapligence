// A large production generated from several places (llg generates it once
// and returns to each caller instead of copying it at every use). The
// result, arguments, waits and the value of each generation must still be
// exact (IEEE 1800-2009 18.17, 18.17.7).
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  int total;
  initial begin
    randsequence (main)
      main : big(1) { $display("first %0d total %0d", big[1], total); }
             big(2) { $display("second %0d total %0d", big[2], total); }
             twice { $display("twice %0d total %0d", twice, total); }
             pick(3) pick(4) ;
      int twice : big(5) big(6) { return big[1] + big[2]; } ;
      int big (int k) : { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; } { total += k; }
                        { #1; return total; } ;
      pick (int k) : big(k) := 1 { $display("pick %0d at %0t total %0d", big, $time, total); }
                   | big(k) := 1 { $display("pick %0d at %0t total %0d", big, $time, total); } ;
    endsequence
    $finish;
  end
endmodule

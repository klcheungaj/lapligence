// SIM-099 regression: a built-in system function called in a void cast
// (IEEE 1800-2009 13.4.1) runs as that function and discards its value; it
// never becomes a VPI task call that needs a registration.
`timescale 1ns / 1ns
module tb;
  logic [7:0] x = 8'b1011_0001;
  integer seed = 5;
  integer old_seed;

  initial begin
    #3;
    void'($countones(x));
    void'($onehot(x));
    void'($onehot0(x));
    void'($isunknown(x));
    void'($clog2(x));
    void'($bits(x));
    void'($itor(x));
    void'($time);
    void'($realtime);
    old_seed = seed;
    void'($random(seed));
    $display("[%0t] after void casts x=%b seed changed=%0d", $time, x, seed != old_seed);
    $finish;
  end
endmodule

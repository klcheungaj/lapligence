// SIM-002: IEEE 1364-2001 17.3.1 example shape: a module prints the time
// scale of an instance nested in another module, and of itself.
`timescale 1 ms / 1 us
module tb;
  b_dat b();
  initial begin
    $printtimescale(b.c1);
    $printtimescale(b);
    $printtimescale;
    $finish(0);
  end
endmodule
`timescale 10 fs / 1 fs
module b_dat;
  c_dat c1 ();
endmodule
`timescale 1 ns / 1 ns
module c_dat;
endmodule

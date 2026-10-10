// SIM-009-A03: SV 13.3.2 forbids referencing automatic task variables in the
// intra-assignment event control of a nonblocking assignment; a `ref` formal
// shares that restriction (SV 13.5.2).
module tb;
  int g, x;
  task automatic t(ref int r, input int n);
    g <= @(posedge r[0]) n;
    #1 if (n > 0) t(r, n - 1);
  endtask
  initial begin
    t(x, 1);
    #1 $finish;
  end
endmodule

// SIM-009-A03: SV 13.5.2 forbids a `ref` argument in any context forbidden for
// automatic variables, and SV 10.4.2 forbids nonblocking assignments to them.
module tb;
  int g;
  task automatic t(ref int r, input int n);
    r <= n;
    #1 if (n > 0) t(r, n - 1);
  endtask
  initial begin
    t(g, 1);
    $finish;
  end
endmodule

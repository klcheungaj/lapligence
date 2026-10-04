// SIM-004 boundary: the lifetime of automatic $monitor arguments after the
// activation returns is unresolved (IEEE 1800-2009 6.21, 21.2.3); the
// frontend rejects tracing an automatic variable, so an automatic string
// monitor never reads storage freed with its activation.
module tb;
  task automatic t();
    string s = "ok";
    $monitor("%s", s);
    #1;
  endtask
  initial begin
    t();
    #1 $finish(0);
  end
endmodule

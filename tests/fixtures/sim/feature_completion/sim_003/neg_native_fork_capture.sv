// SIM-003 boundary: fork-join_none processes that capture an automatic native
// record belong to SIM-010. IEEE 1800-2009 9.3.2 makes the capture legal.
module tb;
  typedef struct {string s; int n;} T;
  task automatic run(input T v);
    fork
      #1 $display("%s %0d", v.s, v.n);
    join_none
  endtask
  initial begin
    run('{"a", 1});
    #2 $finish(0);
  end
endmodule

// $dumpvars with a block, task or function as its scope. IEEE 1800-2009
// 21.7.1.2 names modules and variables as arguments; a tool may take any scope
// of the dumped hierarchy (see readme). The depth counts from the named scope.
module tb;
  reg a = 0;
  int top_v = 0;
  function int f1(int x);
    int k;
    k = x;
    return k + 1;
  endfunction
  function automatic int f0(int x);
    int z;
    z = x;
    return z;
  endfunction
  task t1(input int x);
    int m;
    m = x;
    #1 m = x + 1;
  endtask
  initial begin : blk
    int c;
    begin : inner
      int d;
      begin : deep
        int e;
        e = 1;
        d = 2;
        c = 3;
        $dumpfile("scopes.vcd");
        $dumpvars(1, blk);
        $dumpvars(2, tb.blk.inner);
        $dumpvars(1, t1);
        $dumpvars(0, tb.f1);
        $dumpvars(0, f0);
        #1 e = 2;
        d = 3;
        c = 4;
      end
    end
    t1(3);
    a = f1(1) + f0(2);
    #1 c = 5;
    top_v = 1;
    $display("done %b", a);
    $finish;
  end
endmodule

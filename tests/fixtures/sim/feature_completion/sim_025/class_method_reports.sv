// SIM-025 A01: a deferred report issued in a class method. The report keeps its
// receiver: it reads the object's properties at the end of the slot although
// the method returned and no variable names the object any more (SV 21.2.2,
// 21.2.3). A monitor watches through `this` as a module handle does and keeps
// its object until a new `$monitor` replaces it (SV 21.2.3). Module variables
// read from a method need no receiver and report the same way.
`timescale 1ns/1ns
module tb;
  int g = 3;
  class C;
    int x;
    string s = "a";
    C nx;
    int q[$];
    static int count = 7;
    function new(int v);
      x = v;
    endfunction
    function void show();
      $strobe("%0t show g=%0d x=%0d this.x=%0d count=%0d", $time, g, x, this.x, count);
      x = x + 1;
    endfunction
    function void watch();
      $monitor("%0t watch x=%0d g=%0d", $time, x, g);
    endfunction
    task later();
      #1 $strobe("%0t later x=%0d", $time, x);
      x = 50;
    endtask
    function void forked();
      fork
        begin
          #1 $strobe("%0t branch x=%0d", $time, x);
          x = x + 1;
        end
      join_none
    endfunction
    function void chained();
      $fmonitor(1, "%0t chain nx=%0d q=%0d s=%s", $time, nx.x, q.size(), s);
    endfunction
  endclass
  class D extends C;
    function new(int v);
      super.new(v);
      $strobe("%0t new x=%0d base=%0d", $time, x, super.x);
    endfunction
  endclass
  C c, d;
  D e;
  initial begin
    c = new(1);
    d = new(10);
    c.show();
    // The strobe keeps the object it was issued on.
    c = null;
    #1 d.watch();
    #1 d.x = 11;
    #1 g = 4;
    // The monitor keeps its object too.
    #1 d = null;
    #1 g = 5;
    #1 c = new(2);
    c.later();
    #1 e = new(4);
    e.x = 5;
    #1 c = new(6);
    c.forked();
    c.x = 7;
    #2 c.nx = new(20);
    c.chained();
    #1 c.nx.x = 21;
    #1 c.nx = new(21);
    #1 c.nx = new(22);
    #1 c.q.push_back(1);
    #1 c.s = "zz";
    #1 $finish(0);
  end
endmodule

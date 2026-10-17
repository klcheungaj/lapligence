// class-defects item 1: named blocks inside class methods. Expected lines are
// derived in readme.md.
`timescale 1ns/1ps
class C;
  function void f();
    begin : fb
      int x;
      x = 3;
      $display("%m x=%0d", x);
    end
  endfunction
  function void g();
    begin : outer
      begin : inner
        $display("%m");
      end
    end
  endfunction
  static function int s(int n);
    int acc = 0;
    begin : sb
      for (int i = 0; i < n; i++) begin
        if (i == 3) disable sb;
        acc += i;
      end
    end
    return acc;
  endfunction
  static function void count();
    begin : cb
      static int cnt = 0;
      cnt++;
    end
  endfunction
  static function int counted();
    return count.cb.cnt;
  endfunction
  virtual task t();
    fork : tf
      #1 $display("%0t %m a", $time);
      begin : tb2
        #2 $display("%0t %m b", $time);
      end
    join
  endtask
endclass

class Base;
  int hits;
  virtual function int scan(int n);
    int total = 0;
    begin : outer
      for (int i = 0; i < n; i++) begin : step
        automatic int sq = i * i;
        if (i == 1) disable step;
        if (sq > 20) disable outer;
        total += sq;
        hits++;
      end
    end
    return total;
  endfunction
  function int get_hits();
    return hits;
  endfunction
endclass

class Derived extends Base;
  virtual function int scan(int n);
    begin : again
      int extra;
      extra = super.scan(n) + 100;
      $display("%m extra=%0d", extra);
      return extra;
    end
  endfunction
endclass

class P #(int W = 8);
  logic [W-1:0] v;
  function void fill();
    begin : fb
      logic [W-1:0] t;
      static int calls = 0;
      calls++;
      t = '1;
      v = t;
      $display("%m calls=%0d v=%0h", calls, v);
    end
  endfunction
  static task tick(int n);
    for (int k = 0; k < n; k++) begin : tk
      int c;
      c++;
      #1;
      $display("%0t %m c=%0d", $time, c);
    end
  endtask
endclass

module tb;
  class Wt;
    int id;
    int done_count;
    function new(int i);
      id = i;
      done_count = 0;
    endfunction
    task watch();
      fork : watchers
        begin : w1
          automatic int k = id * 10;
          #5 $display("%0t %m k=%0d", $time, k);
          note();
        end
        begin
          #9 $display("%0t %m late", $time);
          note();
        end
      join
    endtask
    function void note();
      done_count++;
    endfunction
    task kill();
      disable watch.watchers;
    endtask
    function int finished();
      return done_count;
    endfunction
  endclass

  initial begin
    static C c = new;
    static Base b = new;
    static Derived d = new;
    static P #(4) p4 = new;
    static P #(12) p12 = new;
    static Wt x = new(1);
    static Wt y = new(2);
    int r;
    c.f();
    c.g();
    $display("s=%0d", C::s(10));
    C::count();
    C::count();
    $display("counted=%0d", C::counted());
    c.t();
    r = b.scan(10);
    $display("base=%0d hits=%0d", r, b.get_hits());
    b = d;
    $display("derived=%0d", b.scan(3));
    p4.fill();
    p4.fill();
    p12.fill();
    P#(4)::tick(2);
    fork
      x.watch();
      #1 y.watch();
      #8 y.kill();
    join
    $display("%0t watched %0d %0d", $time, x.finished(), y.finished());
    $finish;
  end
endmodule

// Gate-fixes-2 item 4: `%m` in class methods (IEEE 1800-2009 21.2.1.6,
// decision S99-D10). A method prints the name of the class that declares it,
// prefixed by the scope that declares the class (an enclosing class, a
// package, or the module instance or generate block), followed by the method:
// the subroutine that invokes the system task. readme.md derives every line.
`timescale 1ns/1ns

package p;
  function automatic void pf(); $display("pf: %m"); endfunction
  class D;
    function void g(); $display("g: %m"); endfunction
  endclass
endpackage

class C;
  class N;
    function void n(); $display("n: %m"); endfunction
  endclass
  virtual function void f(); $display("f: %m"); endfunction
  task t(); $display("t: %m"); endtask
  static function void s(); $display("s: %m"); endfunction
  function void base_only(); $display("b: %s", $sformatf("%m")); endfunction
endclass

class E extends C;
  virtual function void f(); $display("E.f: %m"); endfunction
endclass

class P #(int W = 8);
  function void h(); $display("h: %m"); endfunction
endclass

module sub;
  class L;
    function void k(); $display("k: %m"); endfunction
  endclass
  initial begin
    static L l = new;
    #1 l.k();
  end
endmodule

module tb;
  sub u1();
  sub u2();
  for (genvar i = 0; i < 2; i++) begin : g
    class G;
      task t(); $display("g: %m"); endtask
    endclass
    initial begin
      static G x = new;
      #2 x.t();
    end
  end
  function void mf(); $display("mf: %m"); endfunction
  initial begin
    static C c = new;
    static E e = new;
    static C ce = e;
    static C::N nn = new;
    static p::D d = new;
    static P#(4) q = new;
    c.f(); c.t(); C::s(); ce.f(); e.base_only(); nn.n();
    d.g(); p::pf(); q.h(); mf();
    #3 $finish;
  end
endmodule

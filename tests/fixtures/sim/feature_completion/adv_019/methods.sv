// ADV-019 catalog gaps, class methods: the VPI application methods.c finds
// class methods by their full names and through class variables, and walks
// the statements of their bodies (IEEE 1800-2009 37.37, 37.56, 37.27). See
// readme.md for the expected output.
`timescale 1ns/1ns
package mpkg;
  class PC;
    int w;
    function int pget();
      return w;
    endfunction
  endclass
endpackage
module tb;
  class C;
    int v;
    function int get(input int d);
      v = v + d;
      return v;
    endfunction
    task run();
      v++;
    endtask
    static function int sf();
      return 7;
    endfunction
    function void nop();
    endfunction
  endclass
  class D extends C;
    task run();
      v += 2;
    endtask
  endclass
  C c;
  D d;
  initial begin
    mpkg::PC p;
    c = new;
    d = new;
    p = new;
    void'(c.get(1));
    c.run();
    d.run();
    c.nop();
    void'(p.pget());
    $methods(1);
    $finish;
  end
endmodule

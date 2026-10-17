// Decision S99-D10: `%m` in a class method names the method below the class
// that declares it, and the class below the scope that declares the class:
// `C.f` for a compilation-unit class, `p.D.g` in package `p`, `tb.L.k` for a
// class declared in module instance `tb`. Packages and module instances are
// named as for their own subroutines (`p.pf`, `tb.mf`).
//
// IEEE 1800-2009 21.2.1.6 (SystemVerilog-1800-2009.txt L36379-36381): "The %m
// format specifier does not accept an argument. Instead, it causes the
// display task to print the hierarchical name of the design element,
// subroutine, named block, or labeled statement that invokes the system task
// containing the format specifier." A class is not part of the design
// hierarchy, so the text does not say how a method is named; the `llg` lines
// show llg's choice and the `mf` line is what every simulator prints. llg
// formerly printed only the class name (`C`).
package p;
  class D;
    function void g(); $display("llg g: %m"); endfunction
  endclass
endpackage

class C;
  function void f(); $display("llg f: %m"); endfunction
endclass

module tb;
  class L;
    function void k(); $display("llg k: %m"); endfunction
  endclass
  function void mf(); $display("mf: %m"); endfunction
  initial begin
    static C c = new;
    static p::D d = new;
    static L l = new;
    c.f();
    d.g();
    l.k();
    mf();
    $finish;
  end
endmodule

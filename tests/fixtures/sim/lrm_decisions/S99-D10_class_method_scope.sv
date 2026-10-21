// Decision S99-D10: `%m` in a class method ends with the name of the method,
// and a module subroutine prints its whole name (`tb.mf`).
//
// IEEE 1800-2009 21.2.1.6 (SystemVerilog-1800-2009.txt L36379-36381): "The %m
// format specifier does not accept an argument. Instead, it causes the
// display task to print the hierarchical name of the design element,
// subroutine, named block, or labeled statement that invokes the system task
// containing the format specifier." A class is not part of the design
// hierarchy, so the text does not say how a method is named; the class lines
// therefore print only the trailing component of `%m` through `u::tail`, and the
// `mf` line is what every simulator prints. llg's full names (the class below
// the scope that declares it: `C.f`, `p.D.g`, `tb.L.k`) are checked by
// llg_policy/S99-D10_full_scope_names.
package u;
  function automatic string tail(string s);
    int e = s.len();
    int i;
    while (e > 0 && s[e - 1] == " ") e--;
    for (i = e - 1; i >= 0; i--) if (s[i] == "." || s[i] == ":") break;
    return s.substr(i + 1, e - 1);
  endfunction
endpackage

package p;
  class D;
    function void g(); $display("g: %s", u::tail($sformatf("%m"))); endfunction
  endclass
endpackage

class C;
  function void f(); $display("f: %s", u::tail($sformatf("%m"))); endfunction
endclass

module tb;
  class L;
    function void k(); $display("k: %s", u::tail($sformatf("%m"))); endfunction
  endclass
  function void mf(); $display("mf: %m"); endfunction
  C c;
  p::D d;
  L l;
  initial begin
    c = new;
    d = new;
    l = new;
    c.f();
    d.g();
    l.k();
    mf();
    $finish;
  end
endmodule

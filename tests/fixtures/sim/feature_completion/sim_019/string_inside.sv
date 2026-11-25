// SV 11.4.13: arrays of strings as `inside` set items, in every storage: package,
// interface, hierarchical, class property, formal, local, parameter and call result.
`timescale 1ns/1ns
package pk; string ps[2] = '{"p0", "p1"}; endpackage
interface ifc; string sa[2]; string sq[$]; string m[2][2]; endinterface
module sub; string sa[2]; string sq[$]; parameter string PS[2] = '{"x", "y"};
  bit g = 0;
  if ("y" inside {PS}) begin : gen initial $display("gen"); end
endmodule
module tb;
  import pk::*;
  ifc i0(); ifc i1();
  sub u(); sub #(.PS('{"a", "b"})) v();
  virtual ifc vif;
  parameter string PT[2][2] = '{'{"a", "b"}, '{"c", "d"}};
  int row = 1;
  string f[2] = '{"a", "b"};
  string sq[$] = '{"q0", "q1"};
  bit ini = "b" inside {f};
  bit ini2 = "q1" inside {sq};
  typedef string sq_t[$];
  function automatic sq_t gs(); return {"ga", "gb"}; endfunction
  bit ini3 = "gb" inside {gs()};
  class B; string pf[2]; protected string pq[$]; static string st[$];
    bit c0 = "ga" inside {gs()};
    function new(); pf[0] = "pf0"; pq.push_back("pq0"); endfunction
    function bit chk(string s); return s inside {this.pf, pq, st}; endfunction
    virtual function string nm(); return "pq0"; endfunction
  endclass
  class D extends B; string own[2]; B other; B arr[2];
    function bit chk2(string s); return s inside {super.pf, own, other.pf, arr[1].pf}; endfunction
  endclass
  D d; B b;
  function automatic bit byref(ref string x[2], ref string y[$], const ref string z[2], input string w[2], string s);
    return s inside {x, y, z, w};
  endfunction
  function automatic bit outp(output string x[2], input string s); x[1] = "o"; return s inside {x}; endfunction
  task automatic t(string s); string l[2]; string dl[]; l[1] = "l1"; dl = new[1]; dl[0] = "dl";
    #1;
    fork begin #1 $display("fork %b %b %b", s inside {l}, "dl" inside {dl}, "l1" inside {l}); end join
  endtask
  string o[2];
  string at[string][2];
  bit wires; assign wires = f[0] inside {i0.sa};
  initial begin
    d = new; b = new; d.other = b; d.arr[1] = new; d.arr[1].pf[0] = "arr";
    d.own[1] = "own1"; B::st.push_back("st0");
    i0.sa[1] = "i01"; i1.sq.push_back("i1q"); i1.m[1][0] = "im"; vif = i1;
    u.sa[0] = "us"; u.sq.push_back("uq");
    #1;
    $display("row %b %b %b", "c" inside {PT[row]}, "a" inside {PT[row]}, "" inside {PT[row + 1]});
    $display("pkg %b %b", "p1" inside {ps}, "p2" inside {pk::ps});
    $display("ifc %b %b %b %b", "i01" inside {i0.sa}, "i1q" inside {i1.sq}, "im" inside {i1.m}, "im" inside {i1.m[1]});
    $display("hier %b %b %b", "us" inside {u.sa}, "uq" inside {u.sq}, "us" inside {v.sa});
    $display("init %b %b %b %b", ini, ini2, ini3, b.c0);
    $display("cls %b %b %b %b %b", d.chk("pf0"), d.chk("pq0"), d.chk("st0"), d.chk2("own1"), d.chk2("arr"));
    $display("ref %b %b %b", byref(f, sq, ps, o, "q1"), byref(f, sq, ps, o, ""), byref(f, sq, ps, o, "n"));
    $display("out %b %s", outp(o, "o"), o[1]);
    t("l1");
    at["k"] = '{"e0", "e1"};
    $display("at %b %b", "e1" inside {at["k"]}, "" inside {at["no"]});
    $display("wire %b", wires);
    i0.sa[0] = "a"; #1 $display("wire %b", wires);
    $display("tern %s", "b" inside {f} ? "T" : "F");
    if (!("b" inside {f}) || "zz" inside {f}) $display("bad");
    assert ("b" inside {f}) else $display("bad");
    foreach (f[i]) if (f[i] inside {"a", sq}) $display("f %0d", i);
    $finish(0);
  end
endmodule

// Decision EO-D2, values selected through computed handles and string
// values: a set with a member that has an effect is scanned member by
// member, and each comparison reads the value again, before its member, also
// where the value is an unpacked member selected through a computed handle
// (`va[k].a[0]`, `ca[k].a[0]`): the handle is evaluated again for each
// comparison, after a member to its left that changes `k`. A value with an
// effect of its own is evaluated once, before the members, and so is a `case
// inside` selector; the members are then compared in order up to the first
// match, a string value as any other.
//
// IEEE 1800-2009 11.4.13 (SystemVerilog-1800-2009.txt L15367-15369): "The
//   members of the set are scanned until a match is found and the operation
//   returns 1'b1. ... The order of evaluation of the expressions and ranges
//   is nondeterministic."
// IEEE 1800-2009 12.5 (L17489): "The case_expression shall be evaluated
//   exactly once and before any of the case_item_expressions."
interface ifc;
  int a[4];
endinterface
class C;
  int a[4];
endclass
module tb;
  ifc i();
  ifc j();
  virtual ifc va[2];
  C ca[2];
  int k, x, n;
  string s, sq[$], log;
  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction
  function automatic string gs(string v);
    log = {log, v, ";"};
    return v;
  endfunction
  function automatic string sv();
    log = {log, "sv;"};
    return s;
  endfunction
  task automatic reset();
    va[0] = i; va[1] = j;
    i.a = '{10, 11, 12, 13}; j.a = '{20, 21, 22, 23};
    ca[0].a = '{10, 11, 12, 13}; ca[1].a = '{20, 21, 22, 23};
    k = 0; n = 0; log = "";
  endtask
  initial begin
    ca[0] = new; ca[1] = new;
    s = "b"; sq = '{"q1", "b"};
    reset(); x = va[k].a[0] inside {bump(), 10};
    $display("A %0d %0d", x, n);
    reset(); x = ca[k].a[0] inside {bump(), 10};
    $display("B %0d %0d", x, n);
    reset(); x = va[k].a[0] inside {bump(), 20};
    $display("C %0d %0d", x, n);
    reset(); x = ca[k].a[0] inside {bump(), 20};
    $display("D %0d %0d", x, n);
    reset(); x = va[k].a[0] inside {10, bump()};
    $display("E %0d %0d", x, n);
    reset(); x = va[k].a[0] inside {[bump():15], 20};
    $display("F %0d %0d", x, n);
    reset(); x = ca[k].a[0] inside {[5:bump() + 9], 20};
    $display("G %0d %0d", x, n);
    reset(); case (va[k].a[0]) inside bump(): $display("H one"); 10: $display("H ten"); 20: $display("H twenty"); default: $display("H none"); endcase
    reset(); x = va[k].a[k] inside {bump(), 21};
    $display("I %0d %0d", x, n);
    reset(); x = sv() inside {gs("a"), gs("b"), gs("c")};
    $display("J %0d %s", x, log);
    reset(); x = sv() inside {"x", gs("b"), "b"};
    $display("K %0d %s", x, log);
    reset(); x = sv() inside {gs("x"), sq};
    $display("L %0d %s", x, log);
    reset(); x = sv() inside {["a":gs("c")], gs("z")};
    $display("M %0d %s", x, log);
    reset(); x = sv() inside {[gs("c"):"d"], gs("z")};
    $display("N %0d %s", x, log);
    reset(); x = s inside {gs("a"), "b"};
    $display("O %0d %s", x, log);
    reset(); x = sv() inside {gs("a"), "z"};
    $display("P %0d %s", x, log);
    $finish;
  end
endmodule

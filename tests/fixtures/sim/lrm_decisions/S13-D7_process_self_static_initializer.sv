// Decision S13-D7: process::self() called while a static declaration
// initializer runs (through a function call) returns the process running
// static initialization, as industry-standard simulators do. It is running
// (status RUNNING) while the initializers execute and FINISHED afterwards; its
// random state can be read, restored and seeded, and acts on the stream the
// initializer draws from. There is one such process per initialization scope:
// initializers of one module instance share it, and each module instance,
// each package and the compilation unit has its own; an initial procedure is a
// different process. The results are collected and printed by the initial
// procedure, so the order in which the scopes initialize does not matter.
//
// IEEE 1800-2009 9.7 (SystemVerilog-1800-2009.txt L12612): "The self()
// function returns a handle to the current process, that is, a handle to the
// process making the call." 6.8 (L5286-5288): static variable initializers
// are executed "before any initial or always procedures are started". The text
// names no process for static initialization.
package pk;
  process h0, h1, h2, h3, h4, h5, h6;
  int code[7];
  function automatic void keep(int slot, process p);
    case (slot)
      0: h0 = p; 1: h1 = p; 2: h2 = p; 3: h3 = p;
      4: h4 = p; 5: h5 = p; default: h6 = p;
    endcase
  endfunction
  // 1: not null, 2: running, 4: get/set_randstate replays, 8: srandom replays.
  function automatic int probe(int slot);
    process p = process::self();
    string s;
    int a, b, result;
    keep(slot, p);
    if (p == null) return 0;
    result = 1;
    if (p.status() == process::RUNNING) result += 2;
    s = p.get_randstate();
    a = $urandom;
    p.set_randstate(s);
    b = $urandom;
    if (a == b) result += 4;
    p.srandom(5);
    a = $urandom;
    p.srandom(5);
    b = $urandom;
    if (a == b) result += 8;
    code[slot] = result;
    return result;
  endfunction
  int pkx = probe(0);
endpackage
class C;
  static int c1 = pk::probe(1);
  static int c2 = pk::probe(2);
endclass
module sub #(parameter int S = 3);
  int s = pk::probe(S);
endmodule
module tb;
  int t1 = pk::probe(5);
  int t2 = pk::probe(6);
  sub #(3) u1();
  sub #(4) u2();
  initial begin
    process i;
    i = process::self();
    $display("codes %0d %0d %0d %0d %0d %0d %0d", pk::code[0], pk::code[1], pk::code[2],
             pk::code[3], pk::code[4], pk::code[5], pk::code[6]);
    $display("same: unit class %0d, module %0d; different: package-unit %0d, instances %0d, instance-parent %0d, package-module %0d, initial %0d",
             pk::h1 == pk::h2, pk::h5 == pk::h6, pk::h0 != pk::h1, pk::h3 != pk::h4,
             pk::h3 != pk::h5, pk::h0 != pk::h5, i != pk::h5);
    $display("after initialization: package %s, module %s", pk::h0.status().name(),
             pk::h5.status().name());
    $finish;
  end
endmodule

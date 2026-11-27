// S09-D12: the order of final procedures in generate blocks, interfaces and
// programs.
// 9.2.3 (SystemVerilog-1800-2009.txt L11258): "All final procedures shall
// execute in an arbitrary order."
// L11285-11288: "SystemVerilog final procedures execute in an arbitrary but
// deterministic sequential order. This is possible because final procedures
// are limited to the legal set of statements allowed for functions.
// NOTE-SystemVerilog does not specify the ordering in which final procedures
// are executed, but implementations should define rules that preserve the
// ordering between runs."
// Decision: the final procedures of interface instances run first, then
// those of module instances, then those of program instances, each kind in
// one hierarchy order. An instance runs its own final procedures in source
// order, then those of its generate blocks (a block's own before those of
// the blocks nested in it, loop iterations in index order). The instances it
// contains follow: those that contain no instance first, then the others;
// each part grouped by definition, in the order the definitions first appear
// (instances written directly in the scope before those in its generate
// blocks), each instance the same way.
`timescale 1ns/1ns
module leaf(input int id);
  final $display("leaf %0d", id);
endmodule
module cap(input int id);
  final $display("cap %0d", id);
endmodule
interface bus(input int id);
  final $display("bus %0d", id);
endinterface
program prog(input int id);
  initial #5;
  final $display("prog %0d", id);
endprogram
module mid(input int id);
  if (1) begin : g
    if (1) begin : inner
      final $display("mid %0d g.inner", id);
      leaf deep(id * 10 + 3);
    end
    final $display("mid %0d g", id);
    leaf own(id * 10 + 2);
  end
  final $display("mid %0d", id);
  leaf first(id * 10 + 1);
  bus b(id * 10 + 4);
endmodule
module host(input int id);
  final $display("host %0d", id);
  prog p(id * 10 + 5);
  cap c(id * 10 + 6);
endmodule
module tb;
  mid m1(1);
  cap c1(2);
  for (genvar i = 0; i < 2; i++) begin : loop
    final $display("tb.loop[%0d]", i);
    leaf lp(90 + i);
  end
  final $display("tb");
  prog p(3);
  leaf l(4);
  host h(5);
  bus b(6);
  mid m2(7);
  cap c2(8);
  initial #1 $finish;
endmodule

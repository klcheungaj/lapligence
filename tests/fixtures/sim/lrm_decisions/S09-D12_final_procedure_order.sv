// S09-D12: the order in which final procedures execute.
// 9.2.3 (SystemVerilog-1800-2009.txt L11258): "All final procedures shall
// execute in an arbitrary order."
// L11285-11288: "SystemVerilog final procedures execute in an arbitrary but
// deterministic sequential order. This is possible because final procedures
// are limited to the legal set of statements allowed for functions.
// NOTE-SystemVerilog does not specify the ordering in which final procedures
// are executed, but implementations should define rules that preserve the
// ordering between runs."
// Decision: the final procedures of a module instance run in source order,
// before those of the instances it contains, which follow in the source
// order of their instantiations, each the same way (the top first, then its
// hierarchy depth first). A final procedure that calls $finish ends the
// simulation, so the ones after it do not run (L11279-11280).
// Generate blocks, interfaces and programs: S09-D12_final_procedure_hierarchy.
`timescale 1ns/1ns
module leaf(input int id);
  final $display("leaf %0d", id);
endmodule
module mid(input int id);
  leaf first(id * 10 + 1);
  final $display("mid %0d a", id);
  leaf second(id * 10 + 2);
  final begin
    $display("mid %0d b", id);
    if (id == 2) $finish;
  end
  leaf third(id * 10 + 3);
endmodule
module tb;
  mid m1(1);
  final $display("tb a");
  mid m2(2);
  initial #1 $finish;
  final $display("tb b");
  mid m3(3);
endmodule

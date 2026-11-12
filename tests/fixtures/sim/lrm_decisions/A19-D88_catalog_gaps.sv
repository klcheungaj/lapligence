// Decision A19-D88: VPI catalog gaps (sparse arrays, class methods, block
// locals, checkers).
// - The locals of a block of an automatic task or function are found by
//   their block path (tb.t.blk.x) and listed by vpiVariables of the block
//   while the block runs, not by vpiAutomatics of the frame, which stays
//   the task's (decision A19-D76). Once the block ended they are not found
//   and the block lists no variables.
// - A method of a class declared in a package is found by its full name
//   (p::C::m); a method with an empty body has no vpiStmt (NULL without an
//   error).
// - IEEE 1800-2009 VPI has no checker object: a checker instance is a
//   vpiModule scope.
//
// IEEE 1800-2009 6.21 (SystemVerilog-1800-2009.txt L6901-6902): "Variables
// declared in an automatic task, function, or block are local in scope,
// default to the lifetime of the call or block"; 37.37 detail 5
// (L60004-60005): "vpiFullName of a task or function declared inside a
// package or class defn shall begin with the full name of the package or
// class defn followed by "::" and immediately followed with the name of the
// task or function"; 37.12 detail 5. Industry-standard behaviour gives a
// block its own frame, still finds a block's locals after the block ended,
// finds no package class method by name and gives an empty body a null
// statement; llg follows the text.
//
// Build A19-D88_catalog_gaps.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d88(input int)` declared as a system task).
package p;
  class C;
    function int m();
      return 1;
    endfunction
    function void nop();
    endfunction
  endclass
endpackage
module tb;
  task automatic t();
    int top_local;
    top_local = 1;
    begin : blk
      int x;
      x = 7;
      $d88(1);
    end
    $d88(2);
  endtask
  initial begin
    p::C c;
    c = new;
    void'(c.m());
    c.nop();
    t();
  end
endmodule

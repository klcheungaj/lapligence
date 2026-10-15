// Decision S39-D10: %m in a subroutine attached to a sequence match item
// names that subroutine; %m in a system task attached to a match item, or in
// an assertion's action block, names the labeled assertion.
//
// IEEE 1800-2009 21.2.1.6 (SystemVerilog-1800-2009.txt L36379-36381): "The
// %m format specifier does not accept an argument. Instead, it causes the
// display task to print the hierarchical name of the design element,
// subroutine, named block, or labeled statement that invokes the system task
// containing the format specifier." 16.3 (L21105-21106): "The optional
// statement label (identifier and colon) creates a named block around the
// assertion statement (or any other statement) and the hierarchical name of
// the scope can be displayed using the %m format specification." 16.11
// (L24187-24190) runs the attached calls in list order in the Reactive
// region. The text does not say which scope a system task written directly in
// the match-item list reports: llg reports the labeled assertion, the same
// scope as its action block (an unlabeled assertion is outside this case, its
// name being tool-assigned, 16.15 L26424-26426).
module tb;
  logic clk = 0, clk2 = 0, a = 1;
  function void fshow(input int tag);
    $display("function %m tag=%0d", tag);
  endfunction
  task automatic tshow(input int tag);
    $display("task %m tag=%0d", tag);
  endtask
  sequence s_match;
    int v;
    (a, v = 7, fshow(v), tshow(v), $display("direct %m"));
  endsequence
  c1: cover property (@(posedge clk) s_match);
  c2: cover property (@(posedge clk2) a) $display("action %m");
  initial begin
    #10 clk = 1;
    #10 clk2 = 1;
    #10 $finish;
  end
endmodule

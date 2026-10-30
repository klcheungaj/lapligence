// Decision S46-D13: statement coverage. Every statement (A.6.4
// statement_item) of the procedural code an instance declares is one
// coverable item: assignments, if, case and loop statements, procedural
// timing controls, subroutine call statements and written begin/fork
// blocks; a null statement and a declaration are not. A statement is
// covered once it starts executing while coverage is collected; a design
// that names SV_COV_STATEMENT in a $coverage_* call collects it from the
// beginning of simulation. $coverage_get_max counts the statements in
// scope, $coverage_get the covered ones; START, STOP and RESET act per
// Table 40-2 scope, a stopped instance counts nothing and RESET makes its
// statements uncovered. Statements of a task count for the instance that
// declares the task.
//
// IEEE 1800-2009 40.2.2 (SystemVerilog-1800-2009.txt L66041-66044):
// "statement coverage: Whether a statement has been executed. Statement is
// anything defined as a statement in the LRM. Covered means it executed at
// least once." L66060-66061: "Over instances or blocks, the coverage number
// is merely the sum of all contained primitives in that instance or
// block." 40.3.2.1 L66123-66125: "`SV_COV_START, if possible, starts
// collecting coverage information in the specified hierarchy. ... Coverage
// is automatically started at the beginning of simulation for all portions
// of the hierarchy enabled for coverage." L66126-66129: "`SV_COV_STOP stops
// collecting coverage information in the specified hierarchy. ...
// `SV_COV_RESET resets all available coverage information in the specified
// hierarchy." Which portions are enabled for coverage, and what executing a
// statement means for a block, are left to the implementation.
module leaf(input logic go);
  int n = 0;
  always @(posedge go) begin
    n++;
    if (n > 1) n = 10;
  end
endmodule

module tb;
  logic go = 0;
  leaf u(go);
  task automatic pulse;
    #1 go = 1'b1;
    #1 go = 1'b0;
  endtask
  initial begin
    $display("t0 u %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, u));
    pulse;
    $display("one u %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, u));
    $display("stop %0d", $coverage_control(`SV_COV_STOP, `SV_COV_STATEMENT, `SV_COV_MODULE, u));
    pulse;
    $display("stopped u %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, u));
    $display("start %0d reset %0d",
             $coverage_control(`SV_COV_START, `SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_control(`SV_COV_RESET, `SV_COV_STATEMENT, `SV_COV_MODULE, "leaf"));
    $display("reset u %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, u));
    pulse;
    $display("two u %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, u),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, u));
    if (n_unused()) $display("never");
    ;
    $display("tb %0d/%0d check %0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_control(`SV_COV_CHECK, `SV_COV_STATEMENT, `SV_COV_HIER, tb));
    $finish;
  end
  function automatic bit n_unused();
    return 0;
  endfunction
endmodule

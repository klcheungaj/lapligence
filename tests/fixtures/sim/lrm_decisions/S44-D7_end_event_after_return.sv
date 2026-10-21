// Decision S44-D7: an `end` block event of a task or function also samples
// when the subroutine leaves through a `return` statement: the sample follows
// the return statement (after a function's return value is set), as it
// follows the last statement of a subroutine that runs to its end. Only a
// disable suppresses it.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32116-32119): "Block
// event expressions that specify the end keyword followed by a hierarchical
// identifier denoting a named block, task, function, or class method shall be
// triggered immediately after the corresponding block, task, function, or
// method executes its last statement. Block event expressions that specify
// the end of execution shall not be triggered if the block, task, function,
// or method is disabled." The text does not say whether a subroutine that
// returns early "executes its last statement".
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  int n;
  int r;
  function automatic int f(int a);
    n = a;
    if (a > 1) return a * 10;
    n = a + 100;
    return 1;
  endfunction
  task automatic t(int a);
    n = a;
    if (a == 5) return;
    n = 7;
  endtask
  covergroup at_f @@(end f);
    option.per_instance = 1;
    coverpoint n { bins two = {2}; bins hundred = {100}; bins never = {0}; }
  endgroup
  covergroup at_t @@(end t);
    option.per_instance = 1;
    coverpoint n { bins five = {5}; bins seven = {7}; }
  endgroup
  at_f gf = new;
  at_t gt = new;
  initial begin
    r = f(2);
    $display("r=%0d f=%0.2f", r, gf.get_inst_coverage());
    r = f(0);
    $display("r=%0d f=%0.2f", r, gf.get_inst_coverage());
    t(5);
    $display("t=%0.2f", gt.get_inst_coverage());
    t(6);
    $display("t=%0.2f", gt.get_inst_coverage());
    $finish;
  end
endmodule

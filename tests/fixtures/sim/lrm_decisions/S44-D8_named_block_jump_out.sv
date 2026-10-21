// Decision S44-D8: an `end` block event of a named block samples only when
// the block runs to the end of its last statement. Leaving it through
// `break` or `continue` does not sample (nor does a disable, as the text
// requires). A `begin` block event samples before the first
// statement of every execution of the block.
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32113-32119): "Block
// event expressions that specify the begin keyword followed by a hierarchical
// identifier denoting a named block, task, function, or class method shall be
// triggered immediately before the corresponding block, task, function, or
// method begins executing its first statement. Block event expressions that
// specify the end keyword followed by a hierarchical identifier denoting a
// named block, task, function, or class method shall be triggered
// immediately after the corresponding block, task, function, or method
// executes its last statement. Block event expressions that specify the end
// of execution shall not be triggered if the block, task, function, or
// method is disabled." The text does not say what a jump out of a named
// block does.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  int n;
  int k;
  covergroup enter @@(begin blk);
    option.per_instance = 1;
    coverpoint n { bins zero = {0}; bins one = {1}; bins two = {2}; bins three = {3}; }
  endgroup
  covergroup leave @@(end blk);
    option.per_instance = 1;
    coverpoint n { bins ran = {30}; bins disabled = {21}; bins continued = {22}; bins broke = {23}; }
  endgroup
  enter ge = new;
  leave gl = new;
  initial begin
    for (k = 0; k < 4; k++) begin
      n = k;
      begin : blk
        n = 20 + k;
        if (k == 1) disable blk;
        if (k == 2) continue;
        if (k == 3) break;
        n = 30 + k;
      end
    end
    $display("enter=%0.2f leave=%0.2f", ge.get_inst_coverage(), gl.get_inst_coverage());
    $finish;
  end
endmodule

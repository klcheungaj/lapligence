// Decision S44-D10: a `return` out of a named block skips the block's `end`
// event, as `break` and `continue` do (S44-D8): the block did not run to the
// end of its last statement. This holds for every named block the return
// leaves, nested ones included. The `end` event of the subroutine itself
// still samples after the return (S44-D7).
//
// IEEE 1800-2009 19.3 (SystemVerilog-1800-2009.txt L32116-32119): "Block
// event expressions that specify the end keyword followed by a hierarchical
// identifier denoting a named block, task, function, or class method shall be
// triggered immediately after the corresponding block, task, function, or
// method executes its last statement. Block event expressions that specify
// the end of execution shall not be triggered if the block, task, function,
// or method is disabled." The text does not say what a jump out of a named
// block does.
module tb;
  int n;
  int r;
  function automatic int f(int k);
    n = 10 + k;
    begin : outer
      n = 20 + k;
      begin : inner
        n = 30 + k;
        if (k == 1) return k;
        n = 40 + k;
      end
      n = 50 + k;
    end
    n = 60 + k;
    return n;
  endfunction
  task automatic t(int k);
    begin : tb_blk
      n = 70 + k;
      if (k == 1) return;
      n = 80 + k;
    end
  endtask
  covergroup at_inner @@(end f.outer.inner);
    coverpoint n { bins ran = {40}; bins returned = {31}; }
  endgroup
  covergroup at_outer @@(end f.outer);
    coverpoint n { bins ran = {50}; bins returned = {31}; }
  endgroup
  covergroup at_f @@(end f);
    coverpoint n { bins ran = {60}; bins returned = {31}; }
  endgroup
  covergroup at_tblk @@(end t.tb_blk);
    coverpoint n { bins ran = {80}; bins returned = {71}; }
  endgroup
  covergroup at_t @@(end t);
    coverpoint n { bins ran = {80}; bins returned = {71}; }
  endgroup
  at_inner gi = new;
  at_outer go = new;
  at_f gf = new;
  at_tblk gb = new;
  at_t gt = new;
  initial begin
    r = f(0);
    r = f(1);
    $display("r=%0d inner=%0.2f outer=%0.2f f=%0.2f", r, gi.get_inst_coverage(), go.get_inst_coverage(), gf.get_inst_coverage());
    t(0);
    t(1);
    $display("blk=%0.2f t=%0.2f", gb.get_inst_coverage(), gt.get_inst_coverage());
    $finish;
  end
endmodule

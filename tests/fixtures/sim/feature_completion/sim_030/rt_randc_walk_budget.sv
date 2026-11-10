// A wide randc variable with too many legal values to list walks its
// permutation and tests each position; LLG_RAND_CYCLIC_WALK bounds the
// positions one draw may test. With no budget the call fails as a resource
// error naming the variable to raise, never as an infeasible constraint.
class walk_c;
  randc bit [19:0] v;
  constraint c { v[3:0] == 4'd5; }
endclass
module tb;
  walk_c w = new;
  int ok;
  initial begin
    ok = w.randomize();
    $display("ok=%0d legal=%0b", ok, w.v[3:0] == 4'd5);
    $finish;
  end
endmodule

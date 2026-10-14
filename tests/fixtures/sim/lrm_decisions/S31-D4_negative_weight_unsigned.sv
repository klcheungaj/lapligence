// Decision S31-D4: a signed weight with its sign bit set is not an error and
// not zero; its bit pattern is the weight, because each summand is unsigned.
// llg also warns on stderr (not compared). The weights are meant to be
// non-negative, so this is an llg policy for out-of-contract values.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31311, L31347):
//   "The randcase_item expressions are non-negative integral values that
//   constitute the branch weights."
//   "... where each summand is unsigned."
//
// neg = -1 is the unsigned weight 4294967295 against a weight of 1, so the
// first branch is taken in all 200 executions (the second has probability
// 2^-32 each). A simulator that reads a negative weight as zero would take
// the second branch every time.
module tb;
  int neg = -1;
  int heavy, light;
  initial begin
    repeat (200) begin
      randcase
        neg : heavy++;
        1 : light++;
      endcase
    end
    $display("negative-weight branch: %0d", heavy);
    $display("weight-1 branch: %0d", light);
    $finish;
  end
endmodule

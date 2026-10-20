// Decision S31-D4: a randcase weight that is negative (a signed weight with
// its sign bit set) is a run-time error that ends the simulation at once: no
// branch is taken, nothing after the randcase runs and final procedures do
// not run (llg: an error on stderr, exit status 1).
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31311, L31347):
//   "The randcase_item expressions are non-negative integral values that
//   constitute the branch weights."
//   "... where each summand is unsigned."
// The text does not say what a negative weight does; llg follows the
// industry-standard behaviour of treating it as an error that ends the run.
module tb;
  int neg = -1;
  int heavy, light;

  final begin
    $display("final block");
  end

  initial begin
    repeat (3)
      randcase
        1 : light++;
        0 : heavy++;
      endcase
    $display("before: %0d %0d", light, heavy);
    randcase
      neg : heavy++;
      1 : light++;
    endcase
    $display("after: %0d %0d", light, heavy);
    $finish;
  end
endmodule

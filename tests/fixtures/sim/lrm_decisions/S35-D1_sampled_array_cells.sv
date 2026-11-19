// llg-test-fixture: tests/fixtures/sim/lrm_decisions/S35-D1_sampled_array_cells.sv
// Decision S35-D1: a selection of a cell of a fixed unpacked array in a
// sampled-value function or a concurrent assertion reads the cell's sampled
// value at the sampled value of its index; an X, Z or out-of-range sampled
// index reads the element's default value.
//
// IEEE 1800-2009 16.5 (SystemVerilog-1800-2009.txt L21447): "All variables in
//   a concurrent assertion use the value sampled in the Preponed region of a
//   time slot".
// 16.6.1 (L21595-21596): "Fixed-size arrays, packed or unpacked, can be used
//   as a whole or as part-selects or as indexed bit-selects or part-selects.
//   The indices can be constants, parameters, or variables."
// 7.4.6 (L8235-8237): "If an index expression is out of the address bounds or
//   if any bit in the address is X or Z, then the index shall be invalid. The
//   result of reading from an array with an invalid index shall return the
//   default uninitialized value for the array element type."
//
// `clk` rises at 5, 15, 25, 35 and 45. `arr[i]` samples 2, 9, 3, 3, 3 (the
// write of 9 at 7 and the index change at 17 are seen from the next tick on),
// so `$past(arr[i])` is 2 (initial), 2, 9, 3, 3 and `$stable(arr[i])` is
// 1, 0, 0, 1, 1. `arr[ix]` samples 2, 9, 9, x (X index at 27), x (index 7 at
// 37), so `$past(arr[ix])` is 2, 2, 9, 9, x. Each tick's blocking increment of
// `arr[3]` precedes `a1` in the time step, but `a1` reads the sampled 4, 5, 6,
// 7, 8 and fails only at 25.
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  logic [3:0] ix = 4'd1;
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    arr[3] = arr[3] + 4'h1;
    $display("%0d past=%h pastx=%h stable=%b", $time, $past(arr[i]), $past(arr[ix]),
             $stable(arr[i]));
  end
  a1: assert property (@(posedge clk) arr[3] != 4'h6) else $display("%0d a1 fail", $time);
  initial begin
    #7 arr[1] = 4'h9;
    #10 i = 2;
    #10 ix = 4'bx;
    #10 ix = 4'd7;
    #10 $finish;
  end
endmodule

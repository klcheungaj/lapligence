// Decision S31-D5: X and Z bits of a weight read as 0, as in any conversion
// of a four-state value to a two-state one, so an all-X weight is 0.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31311): "The randcase_item
// expressions are non-negative integral values that constitute the branch
// weights." The text does not say what a four-state value with X or Z bits
// means; llg uses the 6.11.2 conversion that $urandom_range arguments get.
//
// With the all-X and all-Z weights read as 0, the third branch is the only
// one with a nonzero weight and is taken every time.
module tb;
  logic [3:0] weight_x = 4'bxxxx;
  logic [3:0] weight_z = 4'bzzzz;
  int hit_x, hit_z, hit_three;
  initial begin
    repeat (100) begin
      randcase
        weight_x : hit_x++;
        weight_z : hit_z++;
        3 : hit_three++;
      endcase
    end
    $display("x weight branch: %0d", hit_x);
    $display("z weight branch: %0d", hit_z);
    $display("weight-3 branch: %0d", hit_three);
    $finish;
  end
endmodule

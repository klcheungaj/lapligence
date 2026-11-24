// Decision S07-D7: a select of a two-state vector with an unknown index reads
// 0 and a write through it does nothing, like a position outside the vector;
// the value of an assignment within an expression is the value assigned, also
// when the target position does not exist, and a constant part-select reads 0
// for the positions its vector does not have.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8235-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X
// or Z, then the index shall be invalid." / "Writing to an array with an
// invalid index shall perform no operation."
// IEEE 1800-2009 11.3.6 (L14463-14465): "The semantics of such an assignment
// expression is that of a function that evaluates the right-hand side, casts
// the right-hand side to the left-hand data type, stacks it, updates the
// left-hand side, and returns the stacked value."
module tb;
  bit [7:0] v, mem[2];
  bit [3:0][7:0] pa;
  localparam bit [7:0] P = 8'hff;
  integer xi;
  int i;
  logic [3:0] r;
  initial begin
    v = '1;
    pa = '1;
    mem[0] = '1;
    mem[1] = '1;
    i = 40;
    $display("unknown index %b %b %h %b", v[xi], v[xi +: 4], pa[xi], mem[1][xi]);
    v[xi] = 1'b0;
    pa[xi] = '0;
    $display("unknown index writes %h %h", v, pa);
    r = (v[i] ^= 1'b1);
    $display("assignment value %b", r);
    r = (mem[1][i +: 4] |= 4'b0010);
    $display("element assignment value %b", r);
    r = ++v[i];
    $display("increment value %b", r);
    $display("constant parts %b %b %h", mem[1][6 +: 4], P[6 +: 4], pa[3 +: 2]);
    $display("unchanged %h %h", v, mem[1]);
    $finish;
  end
endmodule

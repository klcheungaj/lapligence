// IEEE 1800-2009 11.4.2 L14532-14533: "For the division or modulus operators,
// if the second operand is a zero, then the entire result value shall be x."
// 11.5.1 L15825: "If the value of addr is out of bounds, then vect[addr]
// returns x." 18.5.12 L30257 makes an ERROR guard fail randomize(), but the
// text is silent on random operands whose value would be x.
// Decision S30-D4 (llg reading): a constraint holds only where it evaluates
// to true, so an assignment that divides by zero, selects outside a vector or
// raises 0 to a negative power is not a solution; randomize() fails only when
// no other assignment is legal. An enclosing false guard absorbs the x, as in
// Figure 18-3.
class ops_c;
  rand bit [3:0] a, b;
  rand bit [3:0] i;
  rand bit [7:0] v;
  constraint d { a / b == 4'd2; }
  constraint s { v[i] == 1'b1; v == 8'h80; }
endclass

module tb;
  ops_c o;
  int fails, bad;

  initial begin
    o = new;
    for (int n = 0; n < 100; n++) begin
      if (!o.randomize()) fails = fails + 1;
      if (o.b == 4'd0 || o.a / o.b != 4'd2) bad = bad + 1;
      if (o.i != 4'd7) bad = bad + 1;
    end
    $display("fails=%0d bad=%0d", fails, bad);
    $finish;
  end
endmodule

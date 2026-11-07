// S07-D13: `min` and `max` give the same element for every order of the
// array when an element (or its `with` key) has an X or Z bit: such a value
// orders below every known value, so `max` skips it and `min` returns it.
// Among several such elements without a `with` clause the first in index
// order is returned.
// 7.12.1 L9293-9294: "min() returns the element with the minimum value or
// whose expression evaluates to a minimum." and "max() returns the element
// with the maximum value or whose expression evaluates to a maximum."
// 7.12.1 L9290-9292: "the with clause (and its expression) may be omitted if
// the relational operators (<, >, ==) are defined for the element type of the
// given array."
// 11.4.4 L14721-14722: "If either operand of a relational operator contains
// an unknown (x) or high-impedance (z) value, then the result shall be a
// 1-bit unknown value (x)." So the text leaves open which element is the
// minimum or maximum when one has such a bit; the decision follows the order
// that makes the result independent of the array's order.
module tb;
  logic [3:0] q[$];
  logic [3:0] r[$];
  logic [3:0] f[3];
  logic [3:0] d[];
  logic [3:0] a_[int];

  function automatic logic [3:0] bump(input logic [3:0] v);
    return v + 4'd1;
  endfunction

  task automatic whole(input logic [3:0] a, b, c);
    q = {a, b, c};
    f = '{a, b, c};
    d = new[3];
    d = '{a, b, c};
    a_.delete();
    a_[5] = a;
    a_[2] = b;
    a_[9] = c;
    r = q.max;
    $write("max %b ", r[0]);
    r = q.min;
    $write("min %b ", r[0]);
    r = f.max;
    $write("fixed %b ", r[0]);
    r = f.min;
    $write("%b ", r[0]);
    r = d.max;
    $write("dyn %b ", r[0]);
    r = d.min;
    $write("%b ", r[0]);
    r = a_.max;
    $write("assoc %b ", r[0]);
    r = a_.min;
    $display("%b", r[0]);
  endtask

  task automatic keyed(input logic [3:0] a, b, c);
    q = {a, b, c};
    d = new[3];
    d = '{a, b, c};
    r = q.max with (item + 1);
    $write("max %b ", r[0]);
    r = q.min with (item + 1);
    $write("min %b ", r[0]);
    r = d.max with (bump(item));
    $write("call %b ", r[0]);
    r = d.min with (bump(item));
    $display("%b", r[0]);
  endtask

  initial begin
    whole(4'b1x00, 4'h3, 4'h9);
    whole(4'h3, 4'b1x00, 4'h9);
    whole(4'h9, 4'h3, 4'b1x00);
    whole(4'hx, 4'h1, 4'h2);
    whole(4'h2, 4'hx, 4'h1);
    whole(4'h2, 4'h1, 4'hz);
    whole(4'b1x00, 4'bxxxx, 4'h3);
    whole(4'bxxxx, 4'b1x00, 4'h3);
    whole(4'h3, 4'bz000, 4'bx000);
    whole(4'bx000, 4'bz000, 4'h3);
    whole(4'bxxxx, 4'bxxxx, 4'hf);
    whole(4'h5, 4'h5, 4'h5);
    keyed(4'b1x00, 4'h3, 4'h9);
    keyed(4'h3, 4'b1x00, 4'h9);
    keyed(4'h9, 4'h3, 4'bz000);
    keyed(4'h2, 4'h7, 4'h4);
    $finish;
  end
endmodule

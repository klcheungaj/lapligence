// IEEE 1800-2009 18.5.7.2 L29939-29940: "In the context of a constraint, an
// array reduction method is treated as an expression iterated over each
// element of the array, joined by the relevant operand for each method."
// 7.12.3 L9403-9404: "The method returns a single value of the same type as
// the array element type or, if specified, the type of the expression in the
// with clause." The text is silent on an array with no elements.
// Decision S30-D6 (industry-standard behaviour): in a constraint, sum() of
// an empty array is 0 and product() is 1; and(), or() and xor() of an empty
// array are evaluation errors, so randomize() returns 0, leaves the random
// variables unchanged and the run continues (llg: an error on stderr, exit
// status 1).
class empty_c;
  rand int e[];
  constraint c { e.size() == 0; e.sum() == 0; e.product() == 1; }
endclass

class and_c;
  rand int e[];
  constraint c { e.size() == 0; e.and() == -1; }
endclass

class or_c;
  rand int e[];
  constraint c { e.size() == 0; e.or() == 0; }
endclass

class xor_c;
  rand int e[];
  constraint c { e.size() == 0; e.xor() == 0; }
endclass

class one_c;
  rand bit [3:0] e[];
  constraint c { e.size() == 0; e.sum() == 4'd1; }
endclass

module tb;
  empty_c e;
  and_c a;
  or_c o;
  xor_c x;
  one_c s;
  initial begin
    e = new;
    e.e = new[2];
    $display("sum and product: ok=%0d size=%0d", e.randomize(), e.e.size());
    a = new;
    a.e = new[2];
    $display("and: ok=%0d size=%0d", a.randomize(), a.e.size());
    o = new;
    o.e = new[2];
    $display("or: ok=%0d size=%0d", o.randomize(), o.e.size());
    x = new;
    x.e = new[2];
    $display("xor: ok=%0d size=%0d", x.randomize(), x.e.size());
    s = new;
    $display("sum1: ok=%0d", s.randomize());
    $finish;
  end
endmodule

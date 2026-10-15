// IEEE 1800-2009 18.5.7.2 L29939-29940: "In the context of a constraint, an
// array reduction method is treated as an expression iterated over each
// element of the array, joined by the relevant operand for each method."
// 7.12.3 L9403-9404: "The method returns a single value of the same type as
// the array element type or, if specified, the type of the expression in the
// with clause." The text is silent on an array with no elements.
// Decision S30-D6 (llg choice): a reduction of an empty array is the
// identity of its operation: sum 0, product 1, and all ones, or 0, xor 0, in
// the result type. Procedural reductions of an empty array give the same
// values in llg.
class empty_c;
  rand int e[];
  constraint c { e.size() == 0; e.sum() == 0; e.product() == 1; e.and() == -1;
                 e.or() == 0; e.xor() == 0; }
endclass

class one_c;
  rand bit [3:0] e[];
  constraint c { e.size() == 0; e.sum() == 4'd1; }
endclass

module tb;
  empty_c e;
  one_c o;
  initial begin
    e = new;
    e.e = new[2];
    $display("identity: ok=%0d size=%0d", e.randomize(), e.e.size());
    o = new;
    $display("sum1: ok=%0d", o.randomize());
    $finish;
  end
endmodule

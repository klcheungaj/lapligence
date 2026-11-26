// Decision SD10-D5: an array item of an unpacked array concatenation whose
// elements are only assignment compatible with the target's elements stands
// for its elements, each converted as by an assignment; an array item of a
// run-time size may fill a fixed-size target, and a result of another size
// is a run-time error.
//
// IEEE 1800-2009 10.10 (SystemVerilog-1800-2009.txt L13838-13847):
//   "An item whose self-determined type is an unpacked array whose
//   slowest-varying dimension's element type is assignment compatible with
//   the element type of the target array shall represent as many elements as
//   exist in that item, arranged in the same left-to-right order as they
//   would appear in the array item itself"
//   "It shall be an error if the size of the resulting array differs from
//   the number of elements in a fixed-size target."
module tb;
  byte b2[2] = '{-1, 2};
  logic [7:0] l2[2] = '{8'hff, 8'bx};
  int i2[2] = '{300, -2};
  int q[$] = '{9, 10};
  int iq[$];
  byte bq[$];
  int f3[3];
  int f4[4];

  initial begin
    iq = {b2, 7};
    $display("%p", iq);
    iq = {l2, 7};
    $display("%p", iq);
    bq = {i2, 7};
    $display("%p", bq);
    f3 = {b2, 7};
    $display("%p", f3);
    f3 = {q, 1};
    $display("%p", f3);
    f4 = {q, 1};
    $display("not reached");
  end
endmodule

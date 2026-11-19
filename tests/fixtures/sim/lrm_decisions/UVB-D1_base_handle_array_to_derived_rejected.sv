// UVB-D1 (negative): an unpacked array of base-class handles stays not
// assignment compatible with an array of derived-class handles.
//
// IEEE 1800-2009 7.6 (SystemVerilog-1800-2009.txt L8422-8424):
//   "A fixed-size unpacked array, dynamic array or queue, or a slice of such an
//    array, shall be assignment compatible with any other such array or slice
//    if all the following conditions are satisfied:
//    -  The element types of source and target shall be equivalent."
// 8.15 (L10196) allows a subclass handle in a base class variable, not the
// reverse, and 7.7 (L8517-8518) applies the 7.6 rule to an array argument.
//
// Decision: only the derived-to-base direction is accepted (with a warning,
// see UVB-D1_derived_handle_array_to_base.sv); base to derived is the
// frontend's located error, also for unrelated classes, an inout or `ref`
// formal, and associative arrays of another index type.
class base;
  int id;
endclass

class derived extends base;
endclass

module tb;
  base b[2];
  derived d[];
  initial begin
    d = b;
    $display("%0d", d.size());
    $finish;
  end
endmodule

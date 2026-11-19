// UVB-D1: an unpacked array of handles of a derived class is not assignment
// compatible with an array of handles of its base class.
//
// IEEE 1800-2009 7.6 (SystemVerilog-1800-2009.txt L8422-8424):
//   "A fixed-size unpacked array, dynamic array or queue, or a slice of such an
//    array, shall be assignment compatible with any other such array or slice
//    if all the following conditions are satisfied:
//    -  The element types of source and target shall be equivalent."
// 6.22.2 (L7106): "a) If two types match, they are equivalent." No other
// rule of 6.22.2 makes two class types equivalent, and 6.22.1 d) (L7051-7052)
// makes a class type match only itself. 7.7 (L8517-8518) applies the same
// rule to an array argument passed by value. 8.15 (L10196) allows a subclass
// handle in a base class variable, one handle at a time.
//
// Decision: llg keeps the frontend's located error for the assignment below
// (and for the same array passed to a base-class array formal, as UVM 1.1d's
// register primer example passes `TABLES` to `uvm_reg_indirect_data::
// configure`). Some simulators accept it and copy each handle; the text
// requires equivalent element types. Copy element by element instead:
// `foreach (d[i]) b[i] = d[i];`.
class base;
  int id;
endclass

class derived extends base;
endclass

module tb;
  derived d[2];
  base b[];
  initial begin
    b = d;
    $display("%0d", b.size());
    $finish;
  end
endmodule

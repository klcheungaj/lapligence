// Decision S40-D3: svDimensions counts the unpacked dimensions of an open
// array plus one when its element is integral (an integer type, bit or logic
// scalar, or packed vector), like $dimensions; dimension 0 is that packed
// part, normalized to [n-1:0]. A query of a dimension the array does not
// have (negative, or above svDimensions minus one) returns 0 from every
// svLeft/svRight/svLow/svHigh/svIncrement/svSize call.
//
// IEEE 1800-2009 H.12.2 (SystemVerilog-1800-2009.txt L73284-73288):
//   "These functions are modeled upon the SystemVerilog array querying
//   functions and use the same semantics (see 20.7). If the dimension is 0,
//   then the query refers to the packed part (which is one-dimensional) of an
//   array, and dimensions > 0 refer to the unpacked part of an array."
// IEEE 1800-2009 20.7 (L34712-34715, L34736-34737):
//   "$dimensions shall return the following: - The total number of dimensions
//   in the array (packed and unpacked, static or dynamic) - 1 for the string
//   data type or any other nonarray type that is equivalent to a simple bit
//   vector type (see 6.11.1) - 0 for any other type" ... "If the first argument to an array query
//   function would cause $dimensions to return 0 or if the second argument is
//   out of range, then 'x shall be returned."
// A C int cannot hold 'x, so llg returns 0 for an absent dimension (llg
// choice).
//
// Build S40-D3_open_array_queries.c into a shared library and load it with
// the simulator's DPI library option.
module tb;
    typedef struct { int a; } rec_t;
    import "DPI-C" function void d3_query(input int c [][], input logic [] v [],
                                          input rec_t r [], input bit [] p);
    int c [1:0][2:4];
    logic [7:4] v [3:5];
    rec_t r [2];
    bit [11:4] p;

    initial begin
        d3_query(c, v, r, p);
        $display("c: $dimensions=%0d", $dimensions(c));
        $finish;
    end
endmodule

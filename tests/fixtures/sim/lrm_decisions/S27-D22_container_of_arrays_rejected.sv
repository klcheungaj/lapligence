// Decision S27-D22: a whole dynamic array, queue or associative array whose
// elements are unpacked arrays, or a slice of its elements, is not a
// memory_name: the call is a compile error located at the argument (here a
// whole associative array of two-element arrays). One element of it (`aa[k]`)
// is a fixed unpacked array and remains a memory_name (S27-D21, S27-D23). The
// expected output is empty: the design does not compile.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37251-37254):
//   "The memory_name can be an unpacked array or a partially indexed
//   multidimensional unpacked array that resolves to a lesser dimensioned
//   unpacked array (see 21.4.3)."
// IEEE 1800-2009 21.4.1 (L37347, L37359-37360):
//   "When loading dynamic arrays and queues, the current size of the array is
//   fixed; the array shall not be resized." "Loading an address creates an
//   element of that index, if it does not already exist."
// IEEE 1800-2009 21.4.3 (L37377-37381) orders the file of a multidimensional
// array row-major, "with each dimension's entries ranging from low to high
// address".
//
// The text describes variable-size arrays of packed words and fixed
// multidimensional arrays, but not a multidimensional memory with a
// variable-size dimension: how an address splits into an element and a cell,
// and where a dump's address specifiers go. llg rejects it, the
// industry-standard behaviour.
module tb;
  logic [7:0] aa [int][2];
  initial begin
    $readmemh("s27_d22.mem", aa);
    $finish;
  end
endmodule

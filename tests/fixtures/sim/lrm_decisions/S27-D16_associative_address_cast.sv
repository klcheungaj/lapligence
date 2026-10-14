// Decision S27-D16: a start, finish or file address of an associative array
// is cast to the index type, as an index expression is. A file address wider
// than the index keeps its low bits, a signed index reads a two's-complement
// address (@fffffffe is -2 for an int index), and an enumerated index takes
// the numeric value even when no member has it.
//
// IEEE 1800-2009 7.8.4 (SystemVerilog-1800-2009.txt L8696-8697):
//   "The index expression shall be evaluated in terms of a cast to the index
//   type, except that an implicit cast from a real or shortreal data type
//   shall be illegal."
// IEEE 1800-2009 21.4.1 (L37360-37362):
//   "When an associative array's index is of an enumerated type, address
//   entries in the pattern file are in numeric format and correspond to the
//   numeric values associated with the elements of the enumerated type."
//
// The text does not say how a file address is converted to the index type;
// llg reads it as an index expression (an llg reading).
module tb;
  typedef enum logic [3:0] { RED = 1, GREEN = 4 } color_t;
  logic [7:0] by_int [int];
  logic [7:0] by_byte [byte unsigned];
  logic [7:0] by_color [color_t];
  integer fd;
  initial begin
    fd = $fopen("s27_d16.mem", "w");
    $fwrite(fd, "@fffffffe\n11\n22\n@1ff\n33\n@4\n44\n55\n");
    $fclose(fd);
    $readmemh("s27_d16.mem", by_int);
    foreach (by_int[k]) $display("int [%0d]=%h", k, by_int[k]);
    $readmemh("s27_d16.mem", by_byte);
    foreach (by_byte[k]) $display("byte [%0d]=%h", k, by_byte[k]);
    $readmemh("s27_d16.mem", by_color, GREEN);
    foreach (by_color[k]) $display("color [%0d]=%h", k, by_color[k]);
    $finish;
  end
endmodule

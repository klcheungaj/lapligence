// Decision S27-D9: a memory file that cannot be opened loads nothing. llg
// reports a warning, the memory keeps its values and the simulation
// continues.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37248-37249):
//   "The filename is an expression that is a string literal, string data
//   type, or an integral data type containing a character string that names
//   the file to be opened."
//
// The text does not say what happens when the file cannot be opened.
module tb;
  logic [7:0] mem [0:1];
  initial begin
    mem = '{8'h12, 8'h34};
    $readmemh("s27_d9_no_such_file.mem", mem);
    $display("mem %h %h", mem[0], mem[1]);
    $finish;
  end
endmodule

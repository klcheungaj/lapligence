// Decision S27-D3: a two-state word reads every x or z digit as 0, so the
// order of extension and conversion does not matter: a leftmost unknown
// digit still pads with zeros in the stored word.
//
// IEEE 1800-2009 21.4.2 (SystemVerilog-1800-2009.txt L37366-37368):
//   "For 2-state integer types, reading proceeds the same as for 4-state
//   variable types (e.g., integer), with the exception that X or Z data are
//   converted to 0."
module tb;
  bit [7:0] hex [0:4];
  int words [0:1];
  integer fd;
  initial begin
    fd = $fopen("s27_d3.hex", "w");
    $fwrite(fd, "x\nz\n1x\nx1\nzX\n");
    $fclose(fd);
    fd = $fopen("s27_d3_int.hex", "w");
    $fwrite(fd, "x0000001\nz\n");
    $fclose(fd);
    $readmemh("s27_d3.hex", hex);
    $readmemh("s27_d3_int.hex", words);
    $display("hex %b %b %b %b %b", hex[0], hex[1], hex[2], hex[3], hex[4]);
    $display("int %h %h", words[0], words[1]);
    $finish;
  end
endmodule

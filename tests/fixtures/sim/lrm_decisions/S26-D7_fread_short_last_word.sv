// Decision S26-D7: when the file ends inside a $fread destination word, the
// bytes read fill the most significant bytes and the rest keep their value.
//
// IEEE 1800-2009 21.3.4.4 (SystemVerilog-1800-2009.txt L37075-37078):
//   "The data in the file shall be read byte by byte to fulfill the request.
//   ... The data are read from the file in a big endian manner; the first
//   byte read is used to fill the most significant location in the memory
//   element."
//
// The text does not cover a word cut short by end of file. llg stores the
// bytes read from the most significant byte down and leaves the remaining
// low bytes unchanged; the return value is the number of bytes read.
module tb;
  integer fd, c;
  logic [23:0] w;
  initial begin
    fd = $fopen("s26_d7.bin", "wb");
    $fwrite(fd, "%c%c", 8'h12, 8'h34);
    $fclose(fd);
    w = 24'haabbcc;
    fd = $fopen("s26_d7.bin", "rb");
    c = $fread(w, fd);
    $fclose(fd);
    $display("c=%0d w=%h", c, w);
    $finish;
  end
endmodule

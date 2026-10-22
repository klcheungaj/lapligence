// Decision S26-D7: when the file ends inside a $fread destination, the bytes
// read fill a variable from its most significant byte down and its other
// bytes become zero, also when no byte was read. In a memory, the element the
// file ends inside keeps its value. The return value counts every byte read.
//
// IEEE 1800-2009 21.3.4.4 (SystemVerilog-1800-2009.txt L37075-37078):
//   "The data in the file shall be read byte by byte to fulfill the request.
//   ... The data are read from the file in a big endian manner; the first
//   byte read is used to fill the most significant location in the memory
//   element."
//
// The text does not cover a destination cut short by end of file. llg
// follows industry-standard behaviour: a variable is filled as if the missing
// bytes were zero; a memory element is stored only when all of its bytes were
// read.
module tb;
  integer fd, c;
  logic [23:0] w;
  logic [15:0] mem [0:2];
  initial begin
    fd = $fopen("s26_d7.bin", "wb");
    $fwrite(fd, "%c%c", 8'h12, 8'h34);
    $fclose(fd);
    w = 24'haabbcc;
    fd = $fopen("s26_d7.bin", "rb");
    c = $fread(w, fd);
    $display("c=%0d w=%h", c, w);
    w = 24'haabbcc;
    c = $fread(w, fd);
    $display("at end c=%0d w=%h", c, w);
    $fclose(fd);
    fd = $fopen("s26_d7.bin", "wb");
    $fwrite(fd, "%c%c%c", 8'h12, 8'h34, 8'h56);
    $fclose(fd);
    mem = '{16'h1111, 16'h2222, 16'h3333};
    fd = $fopen("s26_d7.bin", "rb");
    c = $fread(mem, fd);
    $fclose(fd);
    $display("memory c=%0d %h %h %h", c, mem[0], mem[1], mem[2]);
    $finish;
  end
endmodule

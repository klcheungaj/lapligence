// SIM-026 A01: mixed text and binary input. binary_input.dat (checked in,
// layout in readme.md) holds a text header, ten raw bytes 01..0a and a text
// line; generated.dat is written here with $fwrite %u/%z. Covers $fread into
// packed and memory destinations with start/count, the first byte read
// becoming the most significant byte, a short last word that fills only the
// most significant bytes, %u/%z little-endian 32-bit words and the unread
// suffix after each read (IEEE 1800-2009 21.3.4.3, 21.3.4.4).
module tb;
  integer fd, c, ch;
  string s, line;
  int i;
  logic [15:0] w16;
  logic [7:0] mem[0:3];
  logic [23:0] w24;
  logic [31:0] u32, f32;
  logic [7:0] z8, p8;
  initial begin
    fd = $fopen("binary_input.dat", "rb");
    c = $fscanf(fd, "%s %d", s, i);
    ch = $fgetc(fd);
    $display("A c=%0d s=%s i=%0d ch=%0d", c, s, i, ch);
    c = $fread(w16, fd);
    $display("B c=%0d w16=%h", c, w16);
    c = $fread(mem, fd, 1, 2);
    $display("C c=%0d mem=%h %h %h %h", c, mem[0], mem[1], mem[2], mem[3]);
    c = $fread(w24, fd);
    $display("D c=%0d w24=%h", c, w24);
    c = $fscanf(fd, "%u", u32);
    $display("E c=%0d u32=%h", c, u32);
    c = $fgets(line, fd);
    $display("F c=%0d same=%0d", c, line == "ail line\n");
    ch = $fgetc(fd);
    $display("G ch=%0d eof=%0d", ch, $feof(fd) != 0);
    c = $fread(p8, fd);
    $display("H c=%0d p8=%h", c, p8);
    $fclose(fd);
    fd = $fopen("generated.dat", "wb");
    $fwrite(fd, "%u", 32'h89abcdef);
    $fwrite(fd, "%z", 8'b1x0z_01xz);
    $fwrite(fd, "%u", 16'h1234);
    $fclose(fd);
    fd = $fopen("generated.dat", "rb");
    c = $fread(f32, fd);
    $display("I c=%0d f32=%h", c, f32);
    c = $fscanf(fd, "%z", z8);
    $display("J c=%0d z8=%b", c, z8);
    c = $fread(w16, fd);
    $display("K c=%0d w16=%h", c, w16);
    c = $fread(w24, fd);
    $display("L c=%0d w24=%h", c, w24);
    c = $fscanf(fd, "%u", u32);
    $display("M c=%0d u32=%h", c, u32);
    $fclose(fd);
    $finish;
  end
endmodule

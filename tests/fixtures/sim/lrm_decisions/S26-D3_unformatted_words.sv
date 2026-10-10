// Decision S26-D3: %u and %z read and write 32-bit words, least
// significant word first, each word in little-endian byte order; a read
// that runs out of data assigns nothing.
//
// IEEE 1800-2009 21.3.4.3 Table 21-8 (SystemVerilog-1800-2009.txt
// L36944-36946, L36958-36959):
//   "u Matches unformatted (binary) data. The application shall transfer
//   sufficient data from the input to fill the target variable." / "The data
//   shall be read from the file in the native endian format of the
//   underlying system"
// z (L36988-36990): "in the same endian order as if the PLI was used, the
//   data were in a s_vpi_vecval structure"
//
// llg uses the s_vpi_vecval word layout for both: 32-bit words (an aval word
// then a bval word for %z), least significant word first, little-endian
// bytes (the byte order of llg's supported hosts). A %u or %z conversion that
// meets end of file after some bytes assigns nothing and returns 0 for that
// directive; with no byte left it returns EOF.
module tb;
  integer fd, c, b0, b1, b2, b3;
  logic [31:0] u;
  logic [3:0] z;
  initial begin
    fd = $fopen("s26_d3.bin", "wb");
    $fwrite(fd, "%u", 32'h01020304);
    $fwrite(fd, "%z", 4'b10xz);
    $fwrite(fd, "%c%c", 8'h41, 8'h42);
    $fclose(fd);
    fd = $fopen("s26_d3.bin", "rb");
    b0 = $fgetc(fd);
    b1 = $fgetc(fd);
    b2 = $fgetc(fd);
    b3 = $fgetc(fd);
    $display("u bytes=%h %h %h %h", b0[7:0], b1[7:0], b2[7:0], b3[7:0]);
    c = $fscanf(fd, "%z", z);
    $display("z c=%0d z=%b", c, z);
    u = 32'h55555555;
    c = $fscanf(fd, "%u", u);
    $display("short c=%0d u=%h", c, u);
    c = $fscanf(fd, "%u", u);
    $display("end c=%0d u=%h", c, u);
    $fclose(fd);
    fd = $fopen("s26_d3.bin", "rb");
    c = $fscanf(fd, "%u", u);
    $display("round c=%0d u=%h", c, u);
    $fclose(fd);
    $finish;
  end
endmodule

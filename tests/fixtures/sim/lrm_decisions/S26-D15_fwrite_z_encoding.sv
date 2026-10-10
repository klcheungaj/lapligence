// Decision S26-D15: $fwrite %z writes the s_vpi_vecval encoding, so a Z bit
// is written as aval 0, bval 1 and an X bit as aval 1, bval 1.
//
// IEEE 1800-2009 21.3.4.3 Table 21-8 z (SystemVerilog-1800-2009.txt
// L36988-36990) and 38.15 (L62602):
//   "in the same endian order as if the PLI was used, the data were in a
//   s_vpi_vecval structure" / ab: "00=0, 10=1, 11=X, 01=Z"
//
// Each 32-bit group is an aval word then a bval word, least significant
// group first, little-endian bytes (S26-D3). llg wrote Z as X before.
module tb;
  integer fd, i, b;
  logic [63:0] bytes;
  initial begin
    fd = $fopen("s26_d15.bin", "wb");
    $fwrite(fd, "%z", 8'bxz10_zx01);
    $fclose(fd);
    fd = $fopen("s26_d15.bin", "rb");
    for (i = 0; i < 8; i++) begin
      b = $fgetc(fd);
      bytes[63-8*i-:8] = b[7:0];
    end
    $fclose(fd);
    $display("bytes=%h", bytes);
    $finish;
  end
endmodule

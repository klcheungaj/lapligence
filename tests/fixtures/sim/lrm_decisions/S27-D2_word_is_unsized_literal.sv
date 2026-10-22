// Decision S27-D2: a memory-file number is an unsized, unsigned number of
// the task's radix. Stored in a word it is extended like such a literal: a
// leftmost x or z digit fills the upper bits with x or z, a leftmost known
// digit fills them with zeros (also for a signed word, since the number is
// unsigned). A number may have only as many digits as the word needs (leading
// zeros count); the unused bits of its leftmost digit are dropped, and a
// number with a further digit is an error that ends the load, keeping the
// words loaded before it.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37273-37276):
//   "The numbers shall have neither the length nor the base format
//   specified. ... The unknown value (x or X), the high-impedance value (z or
//   Z), and the underscore ( _ ) can be used in specifying a number as in a
//   SystemVerilog source description."
// IEEE 1800-2009 5.7.1 (L3921-3923, L3927-3929):
//   "If the size of the unsigned number is smaller than the size specified
//   for the literal constant, the unsigned number shall be padded to the left
//   with zeros. If the leftmost bit in the unsigned number is an x or a z,
//   then an x or a z shall be used to pad to the left, respectively."
//   "Unsized unsigned literal constants where the high-order bit is unknown
//   (X or x) or three-state (Z or z) shall be extended to the size of the
//   expression containing the literal constant."
//
// The text does not say what a number wider than the word does; llg follows
// industry-standard behaviour and treats a whole extra digit as an error.
module tb;
  logic [11:0] wide [0:4];
  logic signed [3:0] narrow [0:4];
  logic [7:0] bin [0:4];
  logic [5:0] six [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d2.hex", "w");
    $fwrite(fd, "x\nz\n1x\nx1\nzX\n");
    $fclose(fd);
    fd = $fopen("s27_d2.bin", "w");
    $fwrite(fd, "x\nz\n1x\nx1\nzX\n");
    $fclose(fd);
    $readmemh("s27_d2.hex", wide);
    narrow = '{default: 4'h5};
    $readmemh("s27_d2.hex", narrow);
    $readmemb("s27_d2.bin", bin);
    fd = $fopen("s27_d2_six.hex", "w");
    $fwrite(fd, "ff\nxf\n004\n1\n");
    $fclose(fd);
    six = '{default: 6'h15};
    $readmemh("s27_d2_six.hex", six);
    $display("wide   %b %b %b %b %b", wide[0], wide[1], wide[2], wide[3], wide[4]);
    $display("narrow %b %b %b %b %b", narrow[0], narrow[1], narrow[2], narrow[3], narrow[4]);
    $display("bin    %b %b %b %b %b", bin[0], bin[1], bin[2], bin[3], bin[4]);
    $display("six    %b %b %b %b", six[0], six[1], six[2], six[3]);
    $finish;
  end
endmodule

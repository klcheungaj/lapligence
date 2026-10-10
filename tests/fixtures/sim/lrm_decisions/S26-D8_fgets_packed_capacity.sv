// Decision S26-D8: $fgets into a packed destination reads at most its width
// in whole bytes and right-justifies a shorter line.
//
// IEEE 1800-2009 21.3.4.2 (SystemVerilog-1800-2009.txt L36830-36832):
//   "reads characters from the file specified by fd into the variable str
//   until str is filled, or a newline character is read and transferred to
//   str, or an EOF condition is encountered. If str is not an integral number
//   of bytes in length, the most significant partial byte is not used in
//   order to determine the size."
//
// llg stores the characters as a string literal is stored in a vector
// (5.9): the last character read is the least significant byte and unused
// high bytes are zero.
module tb;
  integer fd, c1, c2;
  logic [8*4-1:0] p;
  logic [8*6+3:0] q;
  initial begin
    fd = $fopen("s26_d8.txt", "w");
    $fwrite(fd, "abcdef\n");
    $fclose(fd);
    fd = $fopen("s26_d8.txt", "r");
    c1 = $fgets(p, fd);
    c2 = $fgets(q, fd);
    $fclose(fd);
    $display("c1=%0d p=%s c2=%0d q=%h", c1, p, c2, q);
    $finish;
  end
endmodule

// Decision S27-D5: a token that is not a binary or hexadecimal number (or an
// `@` address) is an error that ends the load. The words before it stay;
// the cells after it keep their values.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37268-37271):
//   "The text file to be read shall contain only the following:
//   - White space (spaces, newlines, tabs, and formfeeds)
//   - Comments (both types of comment are allowed)
//   - Binary or hexadecimal numbers"
//
// The text does not say what a load does with a file that breaks this rule.
// llg stops at the first malformed token, since the words after it have no
// reliable address, and the simulation continues.
module tb;
  logic [7:0] hex [0:3];
  logic [3:0] bin [0:3];
  integer fd;
  initial begin
    fd = $fopen("s27_d5.hex", "w");
    $fwrite(fd, "11 2g 33 44\n");
    $fclose(fd);
    fd = $fopen("s27_d5.bin", "w");
    $fwrite(fd, "0001 0010 0021 0100\n");
    $fclose(fd);
    hex = '{8'hee, 8'hee, 8'hee, 8'hee};
    bin = '{4'hf, 4'hf, 4'hf, 4'hf};
    $readmemh("s27_d5.hex", hex);
    $readmemb("s27_d5.bin", bin);
    $display("hex %h %h %h %h", hex[0], hex[1], hex[2], hex[3]);
    $display("bin %b %b %b %b", bin[0], bin[1], bin[2], bin[3]);
    $finish;
  end
endmodule

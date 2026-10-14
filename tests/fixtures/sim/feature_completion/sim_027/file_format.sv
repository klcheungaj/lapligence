// SIM-027 A01: memory file grammar (IEEE 1800-2009 21.4): white space
// (spaces, tabs, newlines, form feeds), both comment forms, underscores,
// X/Z digits in either case and `@` hexadecimal addresses. Addresses are
// sparse: unaddressed cells keep their values, and loading continues upward
// after each address. Every expected value below is derived by hand from
// the data files and the cited text.
module tb;
  logic [11:0] words [0:11];
  logic [7:0] bytes [0:4];
  logic [7:0] down [11:0];

  initial begin
    foreach (words[i]) words[i] = 12'h555;
    $readmemh("format.hex", words);
    foreach (words[i]) $display("words[%0d]=%h", i, words[i]);

    foreach (bytes[i]) bytes[i] = 8'hee;
    $readmemb("format.bin", bytes);
    foreach (bytes[i]) $display("bytes[%0d]=%b", i, bytes[i]);

    // An explicit descending range keeps its direction after an address.
    foreach (down[i]) down[i] = 8'h00;
    $readmemh("format.hex", down, 11, 0);
    foreach (down[i]) if (down[i] !== 8'h00) $display("down[%0d]=%h", i, down[i]);
    $finish;
  end
endmodule

// V 17.1.1.7 / SV 21.2.1.8: `%s` prints a packed value as 8-bit ASCII codes,
// right-justified, and never prints leading zero bytes. The first three lines
// are the standards' own examples (V 2.6.3 / SV 11.10 and V 17.1.1).
module tb;
  reg [8*14:1] stringvar;
  reg [15:0] narrow;
  reg [8*8:1] formatted;
  initial begin
    stringvar = "Hello world";
    $display("%s is stored as %h", stringvar, stringvar);
    stringvar = {stringvar, "!!!"};
    $display("%s is stored as %h", stringvar, stringvar);
    $display("%s is ascii value for 101", 101);
    narrow = 16'h0041;
    $display("[%s][%0s]", narrow, narrow);
    $sformat(formatted, "<%s>", narrow);
    $display("%0s", formatted);
  end
endmodule

// SIM-022-A03 nearest illegal: putc writes a const string.
module tb;
  const string name = "abc";
  initial begin
    name.putc(0, "x");
    $display("%s", name);
  end
endmodule

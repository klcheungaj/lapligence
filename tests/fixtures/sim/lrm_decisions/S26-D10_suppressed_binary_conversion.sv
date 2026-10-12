// Decision S26-D10: a suppressed %*u or %*z conversion ends the call.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36879-36881,
// L36944-36945):
//   "an optional assignment suppression character *" / "u Matches
//   unformatted (binary) data. The application shall transfer sufficient
//   data from the input to fill the target variable."
//
// The size of a %u or %z field comes from its destination; a suppressed
// conversion has none. llg treats it as a matching failure: no input is read
// and the call returns the count so far. The file is opened in binary mode
// ("wb"/"rb", 21.3.1), as %u input requires on hosts that map newlines.
module tb;
  integer fd, c;
  logic [7:0] x;
  initial begin
    fd = $fopen("s26_d10.txt", "wb");
    $fwrite(fd, "4 5\n");
    $fclose(fd);
    x = 8'd1;
    fd = $fopen("s26_d10.txt", "rb");
    c = $fscanf(fd, "%*u %d", x);
    $display("c=%0d x=%0d tell=%0d", c, x, $ftell(fd));
    $fclose(fd);
    $finish;
  end
endmodule

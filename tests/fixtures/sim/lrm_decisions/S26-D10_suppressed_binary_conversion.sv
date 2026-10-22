// Decision S26-D10: a suppressed %*u or %*z conversion reads nothing and
// matches nothing; the call goes on with the next directive.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36879-36881,
// L36944-36945):
//   "an optional assignment suppression character *" / "u Matches
//   unformatted (binary) data. The application shall transfer sufficient
//   data from the input to fill the target variable."
//
// The size of a %u or %z field comes from its destination; a suppressed
// conversion has none. llg follows industry-standard behaviour: the field
// has no data, so no input is read, nothing is counted, and the following
// directives are processed. On empty input the next conversion then returns
// EOF. The files are opened in binary mode ("wb"/"rb", 21.3.1), as %u input
// requires on hosts that map newlines.
module tb;
  integer fd, c;
  logic [7:0] x, y;
  initial begin
    fd = $fopen("s26_d10.txt", "wb");
    $fwrite(fd, "4 5\n");
    $fclose(fd);
    x = 8'd1;
    fd = $fopen("s26_d10.txt", "rb");
    c = $fscanf(fd, "%*u %d", x);
    $display("c=%0d x=%0d tell=%0d", c, x, $ftell(fd));
    $fclose(fd);
    x = 1;
    fd = $fopen("s26_d10.txt", "rb");
    c = $fscanf(fd, "%d%*z%d", x, y);
    $display("z c=%0d x=%0d y=%0d tell=%0d", c, x, y, $ftell(fd));
    $fclose(fd);
    fd = $fopen("s26_d10_empty.txt", "wb");
    $fclose(fd);
    fd = $fopen("s26_d10_empty.txt", "rb");
    c = $fscanf(fd, "%*u%d", x);
    $display("empty c=%0d x=%0d", c, x);
    $fclose(fd);
    $finish;
  end
endmodule

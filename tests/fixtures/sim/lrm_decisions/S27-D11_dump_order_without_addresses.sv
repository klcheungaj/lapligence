// Decision S27-D11: $writememh and $writememb write one word per element of
// a fixed or dynamic array, without `@` addresses, from the lowest address
// to the highest whatever the declared direction, so $readmemh/$readmemb
// read the file back unchanged. Explicit start and finish addresses keep
// their order.
//
// IEEE 1800-2009 21.5 (SystemVerilog-1800-2009.txt L37462-37463):
//   "The $writememb and $writememh system tasks can be used to dump memory
//   array (see 7.4.4) contents to files that are readable by $readmemb and
//   $readmemh, respectively."
// IEEE 1800-2009 21.5.3 (L37497-37498):
//   "When $writememb and $writememh write out data corresponding to unpacked
//   or dynamic arrays, address specifiers (@-words) shall not be written to
//   the output file."
// IEEE 1800-2009 21.4.3 (L37377-37378):
//   "The file contents are organized in row-major order, with each
//   dimension's entries ranging from low to high address."
module tb;
  logic [7:0] down [3:0];
  logic [7:0] up [0:3];
  logic [7:0] two [1:0][0:1];
  logic [7:0] flat [0:3];
  integer fd;
  initial begin
    down = '{8'h33, 8'h22, 8'h11, 8'h00};
    two = '{'{8'h10, 8'h11}, '{8'h00, 8'h01}};
    $writememh("s27_d11_down.mem", down);
    $readmemh("s27_d11_down.mem", up);
    $display("down->up %h %h %h %h", up[0], up[1], up[2], up[3]);
    $writememb("s27_d11_range.mem", down, 3, 1);
    up = '{default: 8'hee};
    $readmemb("s27_d11_range.mem", up);
    $display("range %h %h %h %h", up[0], up[1], up[2], up[3]);
    $writememh("s27_d11_two.mem", two);
    $readmemh("s27_d11_two.mem", flat);
    $display("two->flat %h %h %h %h", flat[0], flat[1], flat[2], flat[3]);
    $finish;
  end
endmodule

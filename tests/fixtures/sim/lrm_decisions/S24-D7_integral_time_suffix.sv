// S24-D7: the suffix of $timeformat in an integral value.
// IEEE 1800-2009 20.4.2 (SystemVerilog-1800-2009.txt L34313-34314):
// "timeformat_task ::= $timeformat [ ( units_number , precision_number ,
// suffix_string , minimum_field_width ) ] ;". The text does not restrict the
// type of suffix_string. Decision: it is a string or an integral value. An
// integral value gives one character for each 8-bit group from the most
// significant one, as %s does: a group of zero is a space, a vector of at most
// 8 bits that is zero is no character, and x and z bits read as 0.
`timescale 1ns/1ps
module tb;
  reg [63:0] r;
  reg [31:0] w;
  reg [15:0] x;
  bit [23:0] b;
  struct packed { logic [7:0] a, c; } st;
  byte by;
  int i;
  string s;
  localparam P = "Pq";
  initial begin
    #5;
    r = "ns"; $timeformat(-9, 2, r, 10); $display("<%t>", $time);
    $timeformat(-9, 2, r, 14); $display("<%t>", $time);
    $timeformat(-9, 2, {"n", "x"}, 10); $display("<%t>", $time);
    w = {"a", 8'h0, "b", 8'h0}; $timeformat(-9, 0, w, 10); $display("<%t>", $time);
    x = "z"; x[15:8] = 'x; $timeformat(-9, 0, x, 10); $display("<%t>", $time);
    x = 0; $timeformat(-9, 0, x, 10); $display("<%t>", $time);
    x = 'x; $timeformat(-9, 0, x, 10); $display("<%t>", $time);
    b = " us"; $timeformat(-6, 3, b, 10); $display("<%t>", $time);
    st = "ST"; $timeformat(-9, 0, st, 10); $display("<%t>", $time);
    by = "y"; $timeformat(-9, 0, by, 10); $display("<%t>", $time);
    by = 0; $timeformat(-9, 0, by, 10); $display("<%t>", $time);
    $timeformat(-9, 0, 8'd0, 10); $display("<%t>", $time);
    i = "int"; $timeformat(-9, 0, i, 10); $display("<%t>", $time);
    $timeformat(-9, 0, r[7:0], 10); $display("<%t>", $time);
    $timeformat(-9, 0, P, 10); $display("<%t>", $time);
    s = "str"; $timeformat(-9, 0, s, 10); $display("<%t>", $time);
    $timeformat(-9, 0, string'(r), 10); $display("<%t>", $time);
    $timeformat(-9, 0, "lit", 10); $display("<%t>", $time);
    $timeformat(-9, 0, "", 10); $display("<%t>", $time);
    $finish;
  end
endmodule

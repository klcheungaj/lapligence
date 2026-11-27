// SIM-004: an input string port connected to a string-valued expression other
// than a string variable: an element of a fixed array, an element selected by
// a variable index, a concatenation and an element of a fixed array of two
// dimensions. IEEE 1800-2009 23.3.3.2: "An input port can be connected
// to any expression of a compatible data type. A continuous assignment
// shall be implied when a variable is connected to an input port
// declaration." An element of a queue, dynamic or associative array is no
// such actual (6.21; neg_* in ki_continuous_dynamic_reads).
module child (input string se, input string sv, input string sc, input string sd);
  always @(se) $display("%0d se=%s", $time, se);
  always @(sv) $display("%0d sv=%s", $time, sv);
  always @(sc) $display("%0d sc=%s", $time, sc);
  initial begin
    #5 $display("%0d sd=%s len=%0d", $time, sd, sd.len());
    #2 $display("%0d sd=%s len=%0d", $time, sd, sd.len());
  end
endmodule

module tb;
  string arr [3] = '{"zero", "one", "two"};
  string fa [2] = '{"f0", "f1"};
  string m [2][2] = '{'{"m00", "m01"}, '{"seven", "m11"}};
  string a = "x", b = "y";
  int i = 0;
  child c (.se(arr[1]), .sv(fa[i]), .sc({a, "-", b}), .sd(m[1][0]));
  initial begin
    #1 arr[1] = "uno";
    #1 i = 1;
    #1 fa[1] = "F1";
    #1 a = "z";
    arr[0] = "cero";
    #2 m[1][0] = "siete";
    #2 $finish(0);
  end
endmodule

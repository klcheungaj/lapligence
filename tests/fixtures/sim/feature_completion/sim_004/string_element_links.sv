// SIM-004: an input string port connected to a string-valued expression other
// than a string variable: an element of a fixed array, queue or associative
// array of strings, an element selected by a variable index and a
// concatenation. IEEE 1800-2009 23.3.3.2: "An input port can be connected
// to any expression of a compatible data type. A continuous assignment
// shall be implied when a variable is connected to an input port
// declaration."
module child (input string se, input string sq, input string sc, input string sd);
  always @(se) $display("%0d se=%s", $time, se);
  always @(sq) $display("%0d sq=%s", $time, sq);
  always @(sc) $display("%0d sc=%s", $time, sc);
  initial begin
    #5 $display("%0d sd=%s len=%0d", $time, sd, sd.len());
    #2 $display("%0d sd=%s len=%0d", $time, sd, sd.len());
  end
endmodule

module tb;
  string arr [3] = '{"zero", "one", "two"};
  string q [$] = '{"q0", "q1"};
  string aa [int] = '{7: "seven"};
  string a = "x", b = "y";
  int i = 0;
  child c (.se(arr[1]), .sq(q[i]), .sc({a, "-", b}), .sd(aa[7]));
  initial begin
    #1 arr[1] = "uno";
    #1 i = 1;
    #1 q[1] = "Q1";
    #1 a = "z";
    arr[0] = "cero";
    #2 aa[7] = "siete";
    #2 $finish(0);
  end
endmodule

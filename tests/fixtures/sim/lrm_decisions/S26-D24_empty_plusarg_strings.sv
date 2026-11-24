// S26-D24: plusarg strings that are empty or hold NUL characters. Run with
// the plusargs +abc=5 +77.
// IEEE 1800-2009 21.6 (SystemVerilog-1800-2009.txt L37518-37522): "If a
// variable is used to specify the string, leading nulls in the variable shall
// be ignored and shall not be considered as part of the matching string. ...
// If the prefix of one of the supplied plusargs matches all characters in the
// provided string, the function returns a nonzero integer."
// The text does not say what a string without characters matches. Decision:
// it matches nothing, as the plusarg_string of $value$plusargs too. A NUL
// character that is not leading is a character of the string, so the string
// matches nothing; x and z bits read as 0.
`timescale 1ns/1ps
module tb;
  string q;
  reg [63:0] w;
  reg [23:0] v;
  byte by;
  int i;
  initial begin
    q = ""; w = 0; by = 0;
    $display("empty %0d %0d", $test$plusargs(""), $test$plusargs(q));
    $display("zero %0d %0d %0d", $test$plusargs(w), $test$plusargs(64'h0), $test$plusargs(by));
    w = 'x; $display("x %0d", $test$plusargs(w));
    w = 'z; $display("z %0d", $test$plusargs(w));
    $display("space %0d", $test$plusargs(" "));
    $display("prefix %0d %0d %0d", $test$plusargs("a"), $test$plusargs("ab"), $test$plusargs("abc=5"));
    $display("longer %0d", $test$plusargs("ab "));
    by = "a"; $display("byte %0d", $test$plusargs(by));
    v = "ab"; $display("leading %0d %0d", $test$plusargs(v), $test$plusargs({8'h0, "ab"}));
    v = {8'bx, "ab"}; $display("leading x %0d", $test$plusargs(v));
    v = {"ab", 8'h0}; $display("trailing %0d %0d", $test$plusargs(v), $test$plusargs({"ab", 8'h0}));
    v = {"a", 8'h0, "b"}; $display("inner %0d %0d", $test$plusargs(v), $test$plusargs({"a", 8'h0, "b"}));
    v = {"a", 8'bx, "b"}; $display("inner x %0d", $test$plusargs(v));
    i = 9; $display("value empty %0d %0d", $value$plusargs("%d", i), i);
    q = "%d"; $display("value empty %0d %0d", $value$plusargs(q, i), i);
    q = "zz"; $display("value text %0d %s", $value$plusargs("%s", q), q);
    $display("value %0d %0d", $value$plusargs("abc=%d", i), i);
    i = 9; w = {16'h0, "abc=%d"}; $display("value leading %0d %0d", $value$plusargs(w, i), i);
    i = 9; w = {"abc=", 8'h0, "%d"}; $display("value inner %0d %0d", $value$plusargs(w, i), i);
    $finish;
  end
endmodule

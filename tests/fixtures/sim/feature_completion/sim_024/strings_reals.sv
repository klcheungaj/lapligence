// SIM-024: string and real conversions, string escapes in `%p`, real
// precision and formats held in string variables (SV 21.2.1, 6.16, 5.9).
module tb;
  string s, e, f, t;
  real r;
  shortreal sr;
  initial begin
    s = "hello";
    e = "";
    $display("A|%s|%10s|%-8s|%p|%0p|%s|%p|", s, s, s, s, s, e, e);
    t = "q\"b\\s\tn\001\377";
    $display("B|%p|%0d|", t, t.len());
    s = "a\000b";
    $display("C|%s|%0d|", s, s.len());
    $display("D|%c|%s|", "Z", 24'h414243);
    r = 3.14159265358979;
    sr = 2.5;
    $display("F|%f|%e|%g|%.3f|%10.2f|%-10.2f|%010.2f|", r, r, r, r, r, r, r);
    $display("G|%p|%p|%0p|", r, sr, 0.1);
    $display("H|%p|%p|%p|", 1.0e300, -0.0, 1.0 / 3.0);
    f = "%0d-%s";
    $display("I|%s|", $sformatf(f, 42, "x"));
    $sformat(t, "%h:%0d", 8'h5a, 8'h5a);
    $display("J|%s|", t);
    $swrite(t, "a", 8'd7, "b");
    $display("K|%s|", t);
    $swriteh(t, 8'd7);
    $display("L|%s|", t);
    f = "%p";
    $display("M|%s|", $sformatf(f, s));
    $display("N|%s|", $sformatf("%5.1f|%-6s|", 2.26, "ab"));
    $finish(0);
  end
endmodule

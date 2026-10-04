// SIM-004 string method audit: byte indexing, bounds, embedded zero bytes
// and copy ownership. IEEE 1800-2009 6.16 and 6.16.1-6.16.15: a string never
// holds "\0" and writing 0 to a character is ignored; getc/indexing outside
// 0..len-1 reads 0 and putc there is ignored; substr(i, j) is "" unless
// 0 <= i <= j < len; atoi-family scans leading digits and underscores.
module tb;
  string s, t, u;
  int i;
  byte c;
  initial begin
    s = "a\000b";
    $display("1 %0d [%s]", s.len(), s);
    s = "hello";
    $display("2 %0d %0d %0d %0d", s.getc(0), s.getc(4), s.getc(5), s.getc(-1));
    s.putc(0, "J"); s.putc(5, "x"); s.putc(-1, "y"); s.putc(1, 8'h00);
    $display("3 %s", s);
    s[1] = "E"; c = s[2];
    $display("4 %s %s", s, c);
    $display("5 [%s] [%s] [%s] [%s]", s.substr(1, 3), s.substr(3, 1), s.substr(0, 5), s.substr(4, 4));
    t = s; t.putc(0, "Z");
    $display("6 %s %s", s, t);
    $display("7 %s %s", s.toupper(), s.tolower());
    $display("8 %0d %0d %0d %0d", s.compare("JEllo"), s.compare("JElloz"), s.icompare("jello"), t.compare("Jf"));
    u = "  42xyz"; $display("9 %0d", u.atoi());
    t = "ff"; u = "1_000"; $display("10 %0d %0d", u.atoi(), t.atohex());
    u = "777"; t = "1011"; $display("11 %0d %0d", u.atooct(), t.atobin());
    u.itoa(-12); $display("12 %s", u);
    u.hextoa(255); $display("13 %s", u);
    u.octtoa(8); $display("14 %s", u);
    u.bintoa(5); $display("15 %s", u);
    u = "3.5e2x"; $display("16 %0.1f", u.atoreal());
    u.realtoa(2.5); $display("17 %s", u);
    s = {s, string'(8'h00), "!"}; $display("18 %0d %s", s.len(), s);
    s = {2{"ab"}}; $display("19 %s", s);
    i = 3; s = {i{"x"}}; $display("20 %s", s);
    $finish(0);
  end
endmodule

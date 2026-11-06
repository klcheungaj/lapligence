// llg-test-fixture: tests/fixtures/sim/container_selects/wide_index_strings.sv
// A string index is never truncated (IEEE 1800-2009 6.16, 7.4.6): an index
// beyond 32 bits, an unsigned one above 2**31-1 and an X or Z one are out of
// range. A read returns 0 and a write does nothing. The index arguments of
// getc, putc and substr follow the same rule: they are not converted to the
// `int` formal's low 32 bits, so a wide actual is out of range (getc returns 0,
// putc leaves the string unchanged, substr returns the empty string).
module tb;
  string s;
  logic [32:0] w33;
  logic [63:0] hi;
  logic [3:0] ux;
  int unsigned u32;
  int i3;

  initial begin
    s = "abcdef";
    w33 = 33'h1_0000_0003;
    hi = 64'hFFFF_FFFF_0000_0001;
    ux = 4'bx001;
    u32 = 32'hFFFF_FFFF;
    i3 = 3;
    $display("read %0d %0d %0d %0d %0d", s[i3], s[w33], s[hi], s[ux], s[u32]);
    s[w33] = "Q";
    s[hi] = "R";
    s[ux] = "S";
    s[u32] = "T";
    $display("ignored [%s]", s);
    s[i3] = "Z";
    s[6] = "U";
    $display("valid [%s]", s);
    $display("getc %0d %0d %0d %0d", s.getc(i3), s.getc(w33), s.getc(hi), s.getc(ux));
    s.putc(w33, "Y");
    s.putc(hi, "Y");
    s.putc(ux, "Y");
    s.putc(u32, "Y");
    $display("putc ignored [%s]", s);
    s.putc(i3, "W");
    $display("putc valid [%s]", s);
    $display("substr [%s] [%s] [%s] [%s] [%s]", s.substr(1, 2), s.substr(0, w33), s.substr(w33, w33),
             s.substr(hi, 4), s.substr(ux, 2));
    $finish;
  end
endmodule

// llg-test-fixture: tests/fixtures/sim/container_selects/wide_index_strings.sv
// A string index is never truncated (IEEE 1800-2009 6.16, 7.4.6): an index
// beyond 32 bits, an unsigned one above 2**31-1 and an X or Z one are out of
// range. A read returns 0 and a write does nothing. The method arguments of
// getc and putc are `int` formals (6.16.2, 6.16.3), so a wide actual is
// converted to `int` there like any other call argument.
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
    $display("int formal %0d", s.getc(w33));
    s.putc(w33, "Y");
    $display("int formal [%s]", s);
    $finish;
  end
endmodule

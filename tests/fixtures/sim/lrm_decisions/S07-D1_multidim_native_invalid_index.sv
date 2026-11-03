// Decision S07-D1: an index outside the bounds of a multidimensional fixed
// array of strings, class handles or records with string members, or one with
// an X or Z bit, is an invalid index in every dimension. A read of such an
// element, row or member returns the default of its type, a write does
// nothing, and the run continues.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8235-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X
// or Z, then the index shall be invalid. The result of reading from an array
// with an invalid index shall return the default uninitialized value for the
// array element type. Writing to an array with an invalid index shall perform
// no operation." Some simulators stop with a fatal error at such a read; llg
// follows the clause text.
module tb;
  typedef struct { string s; int n; } rec_t;
  class C; int v = 5; endclass
  string s[2][3];
  rec_t m[3:2][0:1];
  C h[2][2];
  initial begin
    logic [1:0] x;
    int k;
    foreach (s[i, j]) s[i][j] = $sformatf("s%0d%0d", i, j);
    foreach (m[i, j]) m[i][j] = '{s: "m", n: i + j};
    h[1][1] = new;
    x = 2'b1x;
    k = 3;
    $display("read: [%s] [%s] [%s]", s[x][0], s[0][x], s[k][0]);
    $display("record: [%s] %0d %0d", m[1][0].s, m[x][0].n, m[k][0].n);
    $display("handle: %0d %0d", h[0][k] == null, h[1][1].v);
    s[k][0] = "no";
    s[0][x] = "no";
    m[1][1].s = "no";
    $display("write: %s %s %s %s", s[0][0], s[1][0], s[0][1], m[2][1].s);
    $finish;
  end
endmodule

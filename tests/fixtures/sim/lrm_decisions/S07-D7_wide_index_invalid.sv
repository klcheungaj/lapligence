// Decision S07-D7: an index is never truncated. A value that needs more than
// 32 bits, or an unsigned value above 2**31-1, selects the same element as any
// other value outside the bounds: it is an invalid index. A read returns the
// default of the element type (X for a 4-state element, 0 for `int` and a
// string byte, the empty string for `string`), a write does nothing, and the
// run continues. This holds for fixed and dynamic arrays, queues, strings,
// bit-selects and indexed part-selects alike. A queue slice whose bound needs
// more than 32 bits is the empty queue; a bound that fits is clipped to the
// queue (`Q[a:b]` with `a < 0` or `b > $`).
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8235-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X
// or Z, then the index shall be invalid. The result of reading from an array
// with an invalid index shall return the default uninitialized value for the
// array element type. Writing to an array with an invalid index shall perform
// no operation."
// IEEE 1800-2009 7.10.1 (L9039-9045): "If either a or b are 4-state
// expressions containing X or Z values, it yields the empty queue {}." /
// "Q[ a : b ] where a < 0 is the same as Q[ 0 : b ]." / "Q[ a : b ] where b >
// $ is the same as Q[ a : $ ]." / "An invalid index value (i.e., a 4-state
// expression with X's or Z's, or a value that lies outside 0...$) shall cause
// a read operation (e = Q[n]) to return the default initial value for the
// type of queue item" / "An invalid index (i.e., a 4-state expression with
// X's or Z's, or a value that lies outside 0...$+1) shall cause a write
// operation to be ignored and a run-time warning to be issued".
// IEEE 1800-2009 11.5.1 (L15738-15740, L15784-15787): "If the bit-select is
// out of the address bounds or the bit-select is x or z, then the value
// returned by the reference shall be x." / "A part-select that addresses a
// range of bits that are completely out of the address bounds of the vector,
// packed array, packed structure, parameter or concatenation, or a
// part-select that is x or z shall yield the value x when read and shall have
// no effect on the data stored when written."
//
// A value such as 33'h1_0000_0003 lies outside every array, so reading it
// must not return element 3. Some simulators reduce an index to 32 bits and
// select element 3 for most of these forms; llg follows the clause text
// because the value of the index expression, not its low 32 bits, is
// compared with the bounds. The method arguments of `insert`, `delete`,
// `getc`, `putc` and `substr` are `int` formals, so a wider actual is
// converted to `int` there and is not covered by this decision.
module tb;
  logic [7:0] v;
  logic [3:0][7:0] pa;
  int u[-2:9];
  int d[];
  int q[$];
  logic [15:0] lq[$];
  string sq[$];
  string s;
  int s1[$];
  logic [64:0] w65;
  logic [32:0] w33;
  logic [63:0] top;
  logic [63:0] hi;
  int unsigned u32;
  integer k;
  initial begin
    v = 8'hA5;
    pa = 32'h44332211;
    for (k = -2; k <= 9; k++) u[k] = k * 10;
    d = new[4];
    q = {5, 6, 7, 8};
    lq = {16'h11, 16'h22, 16'h33, 16'h44};
    sq = {"a", "b", "c", "d"};
    s = "abcdef";
    for (k = 0; k < 4; k++) d[k] = k + 1;
    w65 = 65'h1_0000_0000_0000_0003;
    w33 = 33'h1_0000_0003;
    top = '1;
    hi = 64'hFFFF_FFFF_0000_0001;
    u32 = 32'hffff_ffff;

    $display("unpacked u[w33]=%0d u[w65]=%0d u[top]=%0d u[u32]=%0d", u[w33], u[w65], u[top], u[u32]);
    $display("dynamic d[w33]=%0d d[w65]=%0d d[hi]=%0d d[u32]=%0d", d[w33], d[w65], d[hi], d[u32]);
    $display("queue int q[w33]=%0d q[w65]=%0d q[top]=%0d q[hi]=%0d q[u32]=%0d",
             q[w33], q[w65], q[top], q[hi], q[u32]);
    $display("queue logic lq[w33]=%h lq[w65]=%h lq[hi]=%h lq[u32]=%h", lq[w33], lq[w65], lq[hi], lq[u32]);
    $display("queue string [%s] [%s] [%s]", sq[w33], sq[w65], sq[hi]);
    $display("string select [%0d] [%0d] [%0d]", s[w33], s[w65], s[hi]);
    $display("bit select v[w33]=%b v[w65]=%b v[top]=%b", v[w33], v[w65], v[top]);
    $display("part select v[w33+:2]=%b v[hi-:2]=%b", v[w33+:2], v[hi-:2]);
    $display("packed element pa[w33]=%h pa[hi]=%h", pa[w33], pa[hi]);

    u[w33] = 777;
    d[w33] = 888;
    q[w33] = 901; q[w65] = 902; q[top] = 903; q[hi] = 904; q[u32] = 905;
    lq[w33] = 16'hA1; lq[hi] = 16'hA2;
    sq[w33] = "X"; sq[hi] = "Y";
    s[w33] = "Q"; s[hi] = "R";
    v[w33] = 1'b0;
    v[w65+:2] = 2'b00;
    pa[w33] = 8'hFF;
    $display("after writes u[3]=%0d d[3]=%0d v=%b pa=%h s=%s", u[3], d[3], v, pa, s);
    $display("after writes q=%0d,%0d,%0d,%0d lq=%h,%h,%h,%h sq=%s%s%s%s",
             q[0], q[1], q[2], q[3], lq[0], lq[1], lq[2], lq[3], sq[0], sq[1], sq[2], sq[3]);

    q[3] = 8;
    q[2] += 1;
    q[w33] += 1;
    q[hi]++;
    $display("compound q=%0d,%0d,%0d,%0d", q[0], q[1], q[2], q[3]);

    s1 = q[0:100];   $display("slice clipped high size=%0d last=%0d", s1.size(), s1[s1.size() - 1]);
    s1 = q[-3:1];    $display("slice clipped low size=%0d first=%0d", s1.size(), s1[0]);
    s1 = q[0:w33];   $display("slice wide high size=%0d", s1.size());
    s1 = q[hi:3];    $display("slice wide low size=%0d", s1.size());
    s1 = q[w33:w33]; $display("slice wide single size=%0d", s1.size());
    $display("queue size %0d", q.size());
    $finish;
  end
endmodule

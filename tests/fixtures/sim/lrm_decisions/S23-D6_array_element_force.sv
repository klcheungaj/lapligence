// Decision S23-D6: an element of a static unpacked array variable, and a
// member of an unpacked structure element of one, is a force and release
// target. The index is evaluated when the force or release statement
// executes and the force stays on that element; a release of an element that
// is not forced, and a force or release whose index is out of range, do
// nothing.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13375):
//   "The left-hand side of the assignment can be a reference to a singular
//   variable, a net, a constant bit-select of a vector net, a constant
//   part-select of a vector net, or a concatenation of these."
// IEEE 1800-2009 6.4 (L4606-4607):
//   "A singular type shall be any data type except an unpacked structure,
//   unpacked union, or unpacked array (see 7.4 on arrays)."
// IEEE 1800-2009 7.4.6 (L8234-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. ... Writing to an
//   array with an invalid index shall perform no operation."
// The text does not say when the index of a force target is evaluated
// (IEEE 1364-2001 9.3.2 excluded every memory word). Industry-standard
// behaviour evaluates it when the statement executes.
typedef struct { int f; int g; } s_t;
module tb;
  int mem[4];
  s_t s[2];
  int i = 1;
  initial begin
    foreach (mem[k]) mem[k] = k * 10;
    force mem[i] = 99;
    #1 $display("1 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 2;
    mem[1] = 5;
    mem[2] = 6;
    #1 $display("2 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    release mem[i];
    mem[1] = 7;
    #1 $display("3 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 1;
    release mem[i];
    mem[1] = 8;
    #1 $display("4 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    i = 7;
    force mem[i] = 77;
    #1 $display("5 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    s[1].f = 2;
    s[1].g = 3;
    force s[1].f = 77;
    s[1].f = 5;
    s[1].g = 4;
    #1 $display("6 %0d %0d", s[1].f, s[1].g);
    release s[1].f;
    s[1].f = 6;
    #1 $display("7 %0d %0d", s[1].f, s[1].g);
    $finish;
  end
endmodule

// Decision S23-D8: a force or release of an array element whose index has X
// or Z bits does nothing.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8234-8237):
//   "If an index expression is out of the address bounds or if any bit in the
//   address is X or Z, then the index shall be invalid. ... Writing to an
//   array with an invalid index shall perform no operation."
// IEEE 1800-2009 10.6.2 (L13373-13375) admits a singular variable, such as
// an element, as a force target; the text does not describe a force whose
// index is invalid, and llg treats it like a write. Industry-standard
// behaviour forces element 0 instead ("1 88 1 2 3", "2 88 1 2 3").
module tb;
  int mem[4];
  logic [3:0] li;
  initial begin
    foreach (mem[k]) mem[k] = k;
    li = 'x;
    force mem[li] = 88;
    #1 $display("1 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    mem[0] = 9;
    release mem[li];
    #1 $display("2 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    $finish;
  end
endmodule

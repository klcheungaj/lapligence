// ADV-026 A02: erroneous strings and arguments are reported by category and
// leave the memory exactly as the rules prescribe; nothing is written outside
// the selected range or the memory. Every case starts from a memory whose
// cells all hold 5a and prints all eight cells. The category (warning for a
// word count that differs from the range, error for a malformed token or an
// address outside the range) follows IEEE 1800-2009 21.4 / IEEE 1364-2001
// 17.2.8; the strings take the format of memory files (D.14 / C.13). The
// tag at the end of each call line lets the test locate the call.
module tb;
  reg [7:0] mem [0:7];
  reg [3:0] unknown;
  integer negative;
  integer i;

  task fresh;
    begin
      for (i = 0; i < 8; i = i + 1) mem[i] = 8'h5a;
    end
  endtask

  task show;
    input [8*3-1:0] tag;
    begin
      $display("%0s %h %h %h %h %h %h %h %h", tag, mem[0], mem[1], mem[2], mem[3], mem[4], mem[5], mem[6], mem[7]);
    end
  endtask

  initial begin
    unknown = 4'bx;
    negative = -1;

    fresh; $sreadmemh(mem, 0, 7, "11 22 1g 33"); show("D1"); // D1
    fresh; $sreadmemh(mem, 0, 7, "44 /* open"); show("D2"); // D2
    fresh; $sreadmemh(mem, 2, 4, "11 @6 22"); show("D3"); // D3
    fresh; $sreadmemh(mem, 0, 7, "11 @9 22"); show("D4"); // D4
    fresh; $sreadmemh(mem, 0, 9, "11"); show("D5"); // D5
    fresh; $sreadmemh(mem, unknown, 3, "11"); show("D6"); // D6
    fresh; $sreadmemh(mem, negative, 3, "11"); show("D7"); // D7
    fresh; $sreadmemh(mem, 0, 7, "11 22"); show("D8"); // D8
    fresh; $sreadmemh(mem, 0, 1, "11 22 33 44"); show("D9"); // D9
    fresh; $sreadmemh(mem, 0, 3, ""); show("D10"); // D10
    fresh; $sreadmemh(mem, 0, 3, "// nothing", "/* nothing */"); show("D11"); // D11
    fresh; $sreadmemh(mem, 0, 3, "11 @zz 22"); show("D12"); // D12
    fresh; $sreadmemb(mem, 0, 3, "1 2"); show("D13"); // D13
    fresh; $sreadmemh(mem, 3, 0, "11 22 33 44 55"); show("D14"); // D14
    $finish;
  end
endmodule

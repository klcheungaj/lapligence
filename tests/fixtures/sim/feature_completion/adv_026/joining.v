// ADV-026 decision S26-D16 and S26-D20 (IEEE 1800-2009 D.14, IEEE 1364-2001
// C.13): the strings of one call, in order, are one memory image in which the
// end of a string separates tokens like a newline. Words, `@` addresses and
// the next-address state continue from one string to the next; a number never
// continues across strings; a line comment ends with its string; a block
// comment that is still open at the end of a string continues in the next
// one. Each case prints the whole memory (cells start at 5a). J2 and J7 leave
// holes after an `@` address, which Verilog-2001 17.2.8 reports as too few
// words and SystemVerilog 21.4 does not.
module tb;
  reg [7:0] mem [0:7];
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
    fresh; $sreadmemh(mem, 0, 3, "1", "2", "3 4"); show("J1");
    fresh; $sreadmemh(mem, 1, 5, "@4 aa", "bb", "@1 cc", "dd"); show("J2");
    fresh; $sreadmemh(mem, 0, 1, "11 // 99", "22"); show("J3");
    fresh; $sreadmemh(mem, 0, 1, "11 /* 99", "88 */ 22"); show("J4");
    fresh; $sreadmemh(mem, 0, 1, "", "11", "", "22", ""); show("J5");
    fresh; $sreadmemb(mem, 0, 2, "1", "10", "1_1"); show("J6");
    fresh; $sreadmemh(mem, 0, 7, "1 2 3", "@6", "4"); show("J7");
    $finish;
  end
endmodule
